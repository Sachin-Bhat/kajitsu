use super::{MenuTree, RawMenuNode};
use crate::tray::{
    actions, backend,
    model::ItemKey,
    ui::{self, TrayUi},
};
use amane::Service;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::Value;

const INTERFACE: &str = "com.canonical.dbusmenu";
static PENDING: LazyLock<Mutex<HashMap<(u64, i32), bool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn fetch(
    connection: &Connection,
    key: &ItemKey,
    path: &str,
    parent: i32,
    notify: bool,
) -> zbus::Result<MenuTree> {
    let proxy = Proxy::new(connection, key.owner.as_str(), path, INTERFACE)?;
    if notify {
        match proxy.call::<_, _, bool>("AboutToShow", &parent) {
            Err(zbus::Error::MethodError(ref name, _, _))
                if name.as_str() == "org.freedesktop.DBus.Error.UnknownMethod" => {}
            result => {
                result?;
            }
        }
    }
    let properties = [
        "label",
        "type",
        "enabled",
        "visible",
        "icon-name",
        "icon-data",
        "toggle-type",
        "toggle-state",
        "children-display",
    ];
    let (revision, raw): (u32, RawMenuNode) =
        proxy.call("GetLayout", &(parent, 1_i32, properties.as_slice()))?;
    if raw.0 != parent {
        return Err(zbus::Error::Failure(
            "Menu returned a different parent".into(),
        ));
    }
    super::decode_layout(revision, raw)
        .map_err(|e| zbus::Error::Failure(format!("Invalid tray menu: {e:?}")))
}
fn send_event(
    connection: &Connection,
    key: &ItemKey,
    path: &str,
    row_id: i32,
    timestamp: u32,
) -> zbus::Result<()> {
    Proxy::new(connection, key.owner.as_str(), path, INTERFACE)?
        .call("Event", &(row_id, "clicked", Value::from(0_i32), timestamp))
}
fn matches(request: u64, key: &ItemKey, path: &str) -> bool {
    TrayUi::read()
        .menu()
        .is_some_and(|m| m.request == request && &m.key == key && m.menu_path == path)
}

pub(crate) fn show(key: ItemKey, path: String, request: u64, parent: i32) {
    request_layout(key, path, request, parent, true);
}

fn request_layout(key: ItemKey, path: String, request: u64, parent: i32, notify: bool) {
    let mut pending = backend::lock(&PENDING);
    if let Some(dirty) = pending.get_mut(&(request, parent)) {
        *dirty = true;
        return;
    }
    pending.insert((request, parent), false);
    drop(pending);
    let queued = actions::enqueue(Box::new(move || {
        let result = (|| {
            let (epoch, connection) = backend::connection()?;
            if !backend::is_current(&key, epoch) || !matches(request, &key, &path) {
                return None;
            }
            let tree = fetch(&connection, &key, &path, parent, notify).ok()?;
            if !backend::is_current(&key, epoch) || !matches(request, &key, &path) {
                return None;
            }
            Some(tree)
        })();
        let dirty = backend::lock(&PENDING)
            .remove(&(request, parent))
            .unwrap_or(false);
        if let Some(tree) = result {
            TrayUi::write().accept_layout(request, parent, tree);
        } else if notify {
            ui::close_request(request);
        }
        if dirty && matches(request, &key, &path) {
            request_layout(key, path, request, parent, false);
        }
    }));
    if !queued {
        backend::lock(&PENDING).remove(&(request, parent));
        if notify {
            ui::close_request(request);
        }
    }
}

pub(crate) fn signal(owner: &str, path: &str) {
    let menu = TrayUi::read()
        .menu()
        .filter(|m| m.key.owner == owner && m.menu_path == path)
        .cloned();
    if let Some(menu) = menu {
        for parent in menu.path {
            request_layout(
                menu.key.clone(),
                menu.menu_path.clone(),
                menu.request,
                parent,
                false,
            );
        }
    }
}

pub(crate) fn activate(request: u64, row_id: i32, timestamp_ms: u32) {
    actions::enqueue(Box::new(move || {
        let ui = TrayUi::read();
        if !ui.row_valid(request, row_id) {
            return;
        }
        let Some(menu) = ui.menu().cloned() else {
            return;
        };
        drop(ui);
        let Some((epoch, connection)) = backend::connection() else {
            return;
        };
        if !backend::is_current(&menu.key, epoch)
            || !TrayUi::read().row_valid(request, row_id)
            || !matches(request, &menu.key, &menu.menu_path)
        {
            return;
        }
        let _ = send_event(
            &connection,
            &menu.key,
            &menu.menu_path,
            row_id,
            timestamp_ms,
        );
        ui::close_request(request);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tray::backend::tests::TestBus;
    use crate::tray::menu::tests::node;
    use std::sync::{Arc, Mutex};

    struct Fixture {
        calls: Arc<Mutex<Vec<String>>>,
    }
    #[zbus::interface(name = "com.canonical.dbusmenu")]
    impl Fixture {
        fn about_to_show(&self, _id: i32) -> bool {
            self.calls.lock().unwrap().push("AboutToShow".into());
            false
        }
        fn get_layout(&self, parent: i32, depth: i32, names: Vec<String>) -> (u32, RawMenuNode) {
            assert_eq!(depth, 1);
            assert!(names.contains(&"label".into()));
            self.calls.lock().unwrap().push("GetLayout".into());
            (1, node(parent, "", vec![node(1, "_Open", vec![])]))
        }
        fn event(&self, id: i32, event: &str, _data: zbus::zvariant::Value<'_>, _timestamp: u32) {
            assert_eq!((id, event), (1, "clicked"));
            self.calls.lock().unwrap().push("Event".into());
        }
    }

    #[test]
    fn about_to_show_precedes_layout_and_event() {
        let bus = TestBus::new();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let server = zbus::blocking::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at(
                "/Menu",
                Fixture {
                    calls: calls.clone(),
                },
            )
            .unwrap()
            .build()
            .unwrap();
        let connection = bus.connect();
        let key = ItemKey {
            owner: server.unique_name().unwrap().to_string(),
            path: "/Item".into(),
            generation: 1,
        };
        let tree = fetch(&connection, &key, "/Menu", 0, true).unwrap();
        assert_eq!(tree.nodes[&1].label, "Open");
        send_event(&connection, &key, "/Menu", 1, 0).unwrap();
        assert_eq!(
            &*calls.lock().unwrap(),
            &["AboutToShow", "GetLayout", "Event"]
        );
    }
}
