use amane::Service;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, MessageIterator, Proxy, connection::Builder};
use zbus::fdo::RequestNameFlags;
use zbus::{MatchRule, Message, message::Type};

use super::model::ItemKey;
use super::registry::Registry;
use super::watcher::{WatcherFreedesktop, WatcherKde};

pub(crate) const WATCHERS: [&str; 2] = [
    "org.kde.StatusNotifierWatcher",
    "org.freedesktop.StatusNotifierWatcher",
];
pub(crate) const WATCHER_PATH: &str = "/StatusNotifierWatcher";

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Default)]
struct Pending {
    wanted: HashSet<ItemKey>,
    inflight: HashSet<ItemKey>,
}

#[derive(Default)]
pub(crate) struct State {
    pub registry: Mutex<Registry>,
    pub hosts: Mutex<HashSet<String>>,
    owned: Mutex<HashSet<ItemKey>>,
    external: Mutex<HashMap<String, HashSet<ItemKey>>>,
    pending: Mutex<Pending>,
    wake: Condvar,
    connection: Mutex<Option<(u64, Connection)>>,
    epoch: AtomicU64,
    reconcile: AtomicBool,
    target_px: AtomicU32,
}

static STATE: LazyLock<Arc<State>> = LazyLock::new(|| Arc::new(State::default()));

impl State {
    pub fn register_owned(
        &self,
        owner: String,
        path: String,
        advertised: String,
    ) -> (ItemKey, bool) {
        let mut registry = lock(&self.registry);
        let new = registry.find(&owner, &path).is_none();
        let key = registry.register(owner, path, advertised);
        drop(registry);
        lock(&self.owned).insert(key.clone());
        self.schedule(key.clone());
        (key, new)
    }

    fn schedule(&self, key: ItemKey) {
        lock(&self.pending).wanted.insert(key);
        self.wake.notify_one();
    }

    fn current(&self, key: &ItemKey, epoch: u64) -> bool {
        self.epoch.load(Ordering::Acquire) == epoch && lock(&self.registry).contains(key)
    }

    fn replace_external(&self, name: &str, keys: HashSet<ItemKey>) {
        lock(&self.external).insert(name.into(), keys);
        let mut live = lock(&self.owned).clone();
        for keys in lock(&self.external).values() {
            live.extend(keys.iter().cloned());
        }
        let mut registry = lock(&self.registry);
        for key in registry.keys() {
            if !live.contains(&key) {
                registry.remove(&key);
            }
        }
    }

    fn remove_owner(&self, owner: &str) -> Vec<String> {
        let mut registry = lock(&self.registry);
        let removed = registry.owner_ids(owner);
        registry.remove_owner(owner);
        drop(registry);
        lock(&self.hosts).remove(owner);
        lock(&self.owned).retain(|key| key.owner != owner);
        for keys in lock(&self.external).values_mut() {
            keys.retain(|key| key.owner != owner);
        }
        lock(&self.pending).wanted.retain(|key| key.owner != owner);
        self.wake.notify_one();
        removed
    }
}

pub(crate) fn connection() -> Option<(u64, Connection)> {
    lock(&STATE.connection).clone()
}

pub(crate) fn is_current(key: &ItemKey, connection_generation: u64) -> bool {
    STATE.current(key, connection_generation)
}

pub(crate) fn live_keys() -> Vec<ItemKey> {
    lock(&STATE.registry).keys()
}

fn open(address: Option<&str>, state: Arc<State>) -> zbus::Result<Connection> {
    let builder = match address {
        Some(address) => Builder::address(address)?,
        None => Builder::session()?,
    };
    builder
        .method_timeout(Duration::from_secs(3))
        .serve_at(WATCHER_PATH, WatcherKde(state.clone()))?
        .serve_at(WATCHER_PATH, WatcherFreedesktop(state))?
        .build()
}

fn owner_of(connection: &Connection, name: &str) -> zbus::Result<String> {
    Proxy::new(
        connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )?
    .call("GetNameOwner", &(name,))
}

fn registration(connection: &Connection, id: &str) -> zbus::Result<(String, String)> {
    let split = id.find('/').unwrap_or(id.len());
    let name = &id[..split];
    let path = if split == id.len() {
        "/StatusNotifierItem"
    } else {
        &id[split..]
    };
    zbus::zvariant::ObjectPath::try_from(path)?;
    Ok((owner_of(connection, name)?, path.into()))
}

fn setup_watchers(
    connection: &Connection,
    state: &Arc<State>,
    known: &mut HashMap<&'static str, String>,
) -> zbus::Result<()> {
    let host = format!("org.kde.StatusNotifierHost-kajitsu-{}", std::process::id());
    let _ = connection.request_name_with_flags(host.as_str(), RequestNameFlags::DoNotQueue.into());
    let own = connection
        .unique_name()
        .map(ToString::to_string)
        .unwrap_or_default();
    for name in WATCHERS {
        let _ = connection.request_name_with_flags(name, RequestNameFlags::DoNotQueue.into());
        let owner = owner_of(connection, name)?;
        let replaced = known.get(name) != Some(&owner);
        if replaced {
            state.replace_external(name, HashSet::new());
        }
        if owner == own {
            lock(&state.hosts).insert(own.clone());
        } else {
            let proxy = Proxy::new(connection, owner.as_str(), WATCHER_PATH, name)?;
            if replaced {
                let _ = proxy.call::<_, _, ()>("RegisterStatusNotifierHost", &(host.as_str(),));
            }
            // Even an unchanged owner may have registered or removed items since its last snapshot.
            if let Ok(ids) = proxy.get_property::<Vec<String>>("RegisteredStatusNotifierItems") {
                let mut keys = HashSet::new();
                for id in ids {
                    if let Ok((item_owner, path)) = registration(connection, &id) {
                        let key = lock(&state.registry).register(item_owner, path, id);
                        state.schedule(key.clone());
                        keys.insert(key);
                    }
                }
                if owner_of(connection, name).ok().as_deref() == Some(owner.as_str()) {
                    state.replace_external(name, keys);
                }
            }
        }
        known.insert(name, owner);
    }
    Ok(())
}

fn signals(connection: Connection, state: Arc<State>, epoch: u64, iterator: MessageIterator) {
    for message in iterator {
        if state.epoch.load(Ordering::Acquire) != epoch {
            break;
        }
        let Ok(message) = message else {
            break;
        };
        handle_signal(&connection, &state, &message);
    }
    state.wake.notify_one();
}

fn handle_signal(connection: &Connection, state: &Arc<State>, message: &Message) {
    let header = message.header();
    let interface = header.interface().map(|i| i.as_str()).unwrap_or_default();
    let member = header.member().map(|m| m.as_str()).unwrap_or_default();
    if interface == "org.freedesktop.DBus" && member == "NameOwnerChanged" {
        if let Ok((name, old, new)) = message.body().deserialize::<(String, String, String)>() {
            if name.starts_with(':') && !old.is_empty() && new.is_empty() {
                let removed = state.remove_owner(&old);
                for id in removed {
                    super::watcher::emit_item_blocking(
                        connection,
                        "StatusNotifierItemUnregistered",
                        &id,
                    );
                }
                let ids = lock(&state.registry).ids();
                super::watcher::emit_properties_blocking(connection, ids);
            }
            if WATCHERS.contains(&name.as_str()) {
                state.reconcile.store(true, Ordering::Release);
                state.wake.notify_one();
            }
        }
    } else if interface == "com.canonical.dbusmenu"
        && matches!(member, "LayoutUpdated" | "ItemsPropertiesUpdated")
    {
        let owner = header.sender().map(|s| s.as_str()).unwrap_or_default();
        let path = header.path().map(|p| p.as_str()).unwrap_or_default();
        super::menu::client::signal(owner, path);
    } else if WATCHERS.contains(&interface) {
        state.reconcile.store(true, Ordering::Release);
        state.wake.notify_one();
    } else if super::item::INTERFACES.contains(&interface)
        || interface == "org.freedesktop.DBus.Properties"
    {
        let owner = header.sender().map(|s| s.as_str()).unwrap_or_default();
        let path = header.path().map(|p| p.as_str()).unwrap_or_default();
        let key = lock(&state.registry).find(owner, path);
        if let Some(key) = key {
            state.schedule(key);
        }
    }
}

fn run_connected(connection: Connection, state: Arc<State>) -> zbus::Result<()> {
    let epoch = state.epoch.fetch_add(1, Ordering::AcqRel) + 1;
    *lock(&state.connection) = Some((epoch, connection.clone()));
    let rule = MatchRule::builder().msg_type(Type::Signal).build();
    // Subscribe before acquiring names or enumerating foreign registries.
    let iterator = MessageIterator::for_match_rule(rule, &connection, Some(512))?;
    let listener_connection = connection.clone();
    let listener_state = state.clone();
    thread::spawn(move || signals(listener_connection, listener_state, epoch, iterator));
    let (sender, receiver) = mpsc::sync_channel::<ItemKey>(64);
    let receiver = Arc::new(Mutex::new(receiver));
    for _ in 0..4 {
        let receiver = receiver.clone();
        let state = state.clone();
        let connection = connection.clone();
        thread::spawn(move || {
            loop {
                let Ok(key) = lock(&receiver).recv() else {
                    break;
                };
                if state.current(&key, epoch)
                    && let Ok(properties) = super::item::read(&connection, &key)
                    && state.current(&key, epoch)
                {
                    let mut properties = properties;
                    properties.tooltip.title = super::item::plain_text(&properties.tooltip.title);
                    properties.tooltip.description =
                        super::item::plain_text(&properties.tooltip.description);
                    let icons = super::icons::resolve(
                        &properties.icons,
                        state.target_px.load(Ordering::Acquire).max(18),
                    );
                    if state.current(&key, epoch) {
                        lock(&state.registry).update(&key, properties, icons);
                    }
                }
                lock(&state.pending).inflight.remove(&key);
                state.wake.notify_one();
            }
        });
    }
    let mut known = HashMap::new();
    let mut output_names = None;
    let mut last_watchers = Instant::now() - Duration::from_secs(2);
    while !connection.is_closed() {
        if state.reconcile.swap(false, Ordering::AcqRel)
            || last_watchers.elapsed() >= Duration::from_secs(1)
        {
            setup_watchers(&connection, &state, &mut known)?;
            if let Ok(outputs) = crate::mango::output_geometries() {
                output_names = Some(outputs.iter().map(|o| o.name.clone()).collect::<Vec<_>>());
                let scale = outputs.iter().map(|o| o.scale).fold(1.0_f32, f32::max);
                let target = (18.0 * scale).ceil().clamp(18.0, 1024.0) as u32;
                if state.target_px.swap(target, Ordering::AcqRel) != target {
                    let keys = lock(&state.registry).keys();
                    for key in keys {
                        state.schedule(key);
                    }
                }
            }
            last_watchers = Instant::now();
        }
        let mut pending = lock(&state.pending);
        let available: Vec<_> = pending
            .wanted
            .difference(&pending.inflight)
            .cloned()
            .collect();
        for key in available {
            match sender.try_send(key.clone()) {
                Ok(()) => {
                    pending.wanted.remove(&key);
                    pending.inflight.insert(key);
                }
                Err(mpsc::TrySendError::Full(_)) => break,
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err(zbus::Error::Failure("Tray refresh workers stopped".into()));
                }
            }
        }
        drop(pending);
        let snapshot = lock(&state.registry).snapshot();
        if let Some(outputs) = &output_names {
            let keys = snapshot
                .items
                .iter()
                .map(|i| i.key.clone())
                .collect::<Vec<_>>();
            super::ui::reconcile(
                &keys,
                outputs,
                crate::settings::Settings::read().flag("bar_systray"),
            );
        }
        super::publish(snapshot);
        let pending = lock(&state.pending);
        drop(
            state
                .wake
                .wait_timeout(pending, Duration::from_millis(100))
                .unwrap_or_else(PoisonError::into_inner),
        );
    }
    Ok(())
}

pub(crate) fn run() {
    let mut delay = Duration::from_millis(250);
    let mut last_error = String::new();
    loop {
        match open(None, STATE.clone()) {
            Ok(connection) => {
                delay = Duration::from_millis(250);
                let _ = run_connected(connection, STATE.clone());
                STATE.epoch.fetch_add(1, Ordering::AcqRel);
                *lock(&STATE.connection) = None;
                lock(&STATE.registry).clear();
                lock(&STATE.owned).clear();
                lock(&STATE.external).clear();
                lock(&STATE.hosts).clear();
                *lock(&STATE.pending) = Pending::default();
                super::publish(lock(&STATE.registry).snapshot());
                super::ui::close();
            }
            Err(error) => {
                let error = error.to_string();
                if error != last_error {
                    eprintln!("kajitsu: tray bus unavailable: {error}");
                    last_error = error;
                }
            }
        }
        thread::sleep(delay);
        delay = (delay * 2).min(Duration::from_secs(5));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};
    use zbus::blocking::{Connection, Proxy, connection::Builder};

    pub(crate) struct TestBus {
        pub address: String,
        child: Child,
    }
    impl TestBus {
        pub(crate) fn new() -> Self {
            let mut child = Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let mut address = String::new();
            BufReader::new(child.stdout.take().unwrap())
                .read_line(&mut address)
                .unwrap();
            Self {
                address: address.trim().into(),
                child,
            }
        }
        pub(crate) fn connect(&self) -> Connection {
            Builder::address(self.address.as_str())
                .unwrap()
                .method_timeout(Duration::from_secs(3))
                .build()
                .unwrap()
        }
    }
    impl Drop for TestBus {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    #[test]
    fn watcher_owns_or_hosts_without_replacement() {
        let bus = TestBus::new();
        let first_state = Arc::new(State::default());
        let first = open(Some(&bus.address), first_state.clone()).unwrap();
        let mut first_names = HashMap::new();
        setup_watchers(&first, &first_state, &mut first_names).unwrap();
        let iterator = MessageIterator::for_match_rule(
            MatchRule::builder().msg_type(Type::Signal).build(),
            &first,
            Some(128),
        )
        .unwrap();
        let signal_connection = first.clone();
        let signal_state = first_state.clone();
        thread::spawn(move || signals(signal_connection, signal_state, 0, iterator));
        let owner_before = owner_of(&first, WATCHERS[0]).unwrap();
        assert_eq!(owner_before, first.unique_name().unwrap().to_string());
        let second_state = Arc::new(State::default());
        let second = open(Some(&bus.address), second_state.clone()).unwrap();
        setup_watchers(&second, &second_state, &mut HashMap::new()).unwrap();
        assert_eq!(owner_of(&second, WATCHERS[0]).unwrap(), owner_before);
        assert_eq!(second.method_timeout(), Some(Duration::from_secs(3)));
        let app = bus.connect();
        app.request_name("org.kde.StatusNotifierItem.fixture")
            .unwrap();
        for interface in WATCHERS {
            let proxy = Proxy::new(&app, interface, WATCHER_PATH, interface).unwrap();
            assert_eq!(proxy.get_property::<i32>("ProtocolVersion").unwrap(), 0);
            assert!(
                proxy
                    .get_property::<bool>("IsStatusNotifierHostRegistered")
                    .unwrap()
            );
            proxy
                .call::<_, _, ()>(
                    "RegisterStatusNotifierItem",
                    &"org.kde.StatusNotifierItem.fixture",
                )
                .unwrap();
            proxy
                .call::<_, _, ()>("RegisterStatusNotifierItem", &"/AnotherItem")
                .unwrap();
            proxy
                .call::<_, _, ()>("RegisterStatusNotifierItem", &"/AnotherItem")
                .unwrap();
            let ids = proxy
                .get_property::<Vec<String>>("RegisteredStatusNotifierItems")
                .unwrap();
            assert_eq!(ids.len(), 2);
            assert!(
                ids.iter()
                    .any(|id| id == "org.kde.StatusNotifierItem.fixture/StatusNotifierItem")
            );
            assert!(ids.iter().any(|id| id.ends_with("/AnotherItem")));
        }
        let watcher = Proxy::new(&second, WATCHERS[0], WATCHER_PATH, WATCHERS[0]).unwrap();
        let mut unregistered = watcher
            .receive_signal("StatusNotifierItemUnregistered")
            .unwrap();
        let (send, receive) = mpsc::channel();
        thread::spawn(move || {
            let ids: Vec<String> = unregistered
                .by_ref()
                .take(2)
                .map(|m| m.body().deserialize().unwrap())
                .collect();
            send.send(ids).unwrap();
        });
        drop(app);
        let ids = receive.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(ids.contains(&"org.kde.StatusNotifierItem.fixture/StatusNotifierItem".into()));
        assert!(lock(&first_state.registry).ids().is_empty());
    }
}
