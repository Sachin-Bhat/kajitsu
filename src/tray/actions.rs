use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex, mpsc};
use std::thread;

use zbus::blocking::{Connection, Proxy};

use super::backend::{self, lock};
use super::model::{ItemKey, SlotAnchor};
use crate::mango::OutputGeometry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScrollAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemAction {
    Activate,
    ContextMenu,
    SecondaryActivate,
    Scroll { delta: i32, axis: ScrollAxis },
}

type Job = Box<dyn FnOnce() + Send + 'static>;

struct ActionQueue {
    sender: mpsc::SyncSender<Job>,
}
impl ActionQueue {
    fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Job>(64);
        thread::spawn(move || {
            for job in receiver {
                job();
            }
        });
        Self { sender }
    }
    fn enqueue(&self, job: Job) -> bool {
        self.sender.try_send(job).is_ok()
    }
}

static QUEUE: LazyLock<ActionQueue> = LazyLock::new(ActionQueue::new);

pub(crate) fn enqueue(job: Job) -> bool {
    QUEUE.enqueue(job)
}

pub(crate) fn submit(item: ItemKey, action: ItemAction, anchor: SlotAnchor) -> bool {
    enqueue(Box::new(move || {
        let Some((epoch, connection)) = backend::connection() else {
            return;
        };
        if !backend::is_current(&item, epoch) {
            return;
        }
        let Ok(outputs) = crate::mango::output_geometries() else {
            return;
        };
        let _ = execute(&connection, &item, action, &anchor, &outputs, || {
            backend::is_current(&item, epoch)
        });
    }))
}

fn coordinates(outputs: &[OutputGeometry], anchor: &SlotAnchor) -> Option<(i32, i32)> {
    let output = outputs.iter().find(|o| o.name == anchor.output)?;
    let x = anchor.x + anchor.width / 2.0;
    let y = anchor.y + anchor.height / 2.0;
    if !x.is_finite() || !y.is_finite() || output.width == 0 || output.height == 0 {
        return None;
    }
    let x = f64::from(output.x) + f64::from(x.clamp(0.0, output.width.saturating_sub(1) as f32));
    let y = f64::from(output.y) + f64::from(y.clamp(0.0, output.height.saturating_sub(1) as f32));
    if !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&x)
        || !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&y)
    {
        return None;
    }
    Some((x.round() as i32, y.round() as i32))
}

fn execute(
    connection: &Connection,
    key: &ItemKey,
    action: ItemAction,
    anchor: &SlotAnchor,
    outputs: &[OutputGeometry],
    current: impl Fn() -> bool,
) -> zbus::Result<()> {
    let Some((x, y)) = coordinates(outputs, anchor) else {
        return Ok(());
    };
    for interface in super::item::INTERFACES {
        if !current() {
            return Ok(());
        }
        let proxy = Proxy::new(connection, key.owner.as_str(), key.path.as_str(), interface)?;
        let result = match action {
            ItemAction::Activate => proxy.call::<_, _, ()>("Activate", &(x, y)),
            ItemAction::ContextMenu => proxy.call::<_, _, ()>("ContextMenu", &(x, y)),
            ItemAction::SecondaryActivate => proxy.call::<_, _, ()>("SecondaryActivate", &(x, y)),
            ItemAction::Scroll { delta, axis } => proxy.call::<_, _, ()>(
                "Scroll",
                &(
                    delta,
                    match axis {
                        ScrollAxis::Horizontal => "horizontal",
                        ScrollAxis::Vertical => "vertical",
                    },
                ),
            ),
        };
        match result {
            Err(zbus::Error::MethodError(ref name, _, _))
                if matches!(
                    name.as_str(),
                    "org.freedesktop.DBus.Error.UnknownInterface"
                        | "org.freedesktop.DBus.Error.UnknownMethod"
                ) =>
            {
                continue;
            }
            result => return result,
        }
    }
    Ok(())
}

#[derive(Default)]
struct ScrollAccumulator {
    x: f32,
    y: f32,
}
impl ScrollAccumulator {
    fn push(&mut self, scroll: amane::Scroll) -> Vec<ItemAction> {
        let mut actions = Vec::new();
        for (total, delta, axis) in [
            (&mut self.x, scroll.x, ScrollAxis::Horizontal),
            (&mut self.y, scroll.y, ScrollAxis::Vertical),
        ] {
            if !delta.is_finite() {
                continue;
            }
            let value = *total + delta;
            if !value.is_finite() {
                continue;
            }
            let steps = value.trunc().clamp(-1024.0, 1024.0) as i32;
            *total = value.fract();
            if steps != 0 {
                actions.push(ItemAction::Scroll {
                    delta: steps * 120,
                    axis,
                });
            }
        }
        actions
    }
}

static SCROLL: LazyLock<Mutex<HashMap<ItemKey, ScrollAccumulator>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn scroll(item: ItemKey, scroll: amane::Scroll, anchor: SlotAnchor) {
    let live: HashSet<_> = backend::live_keys().into_iter().collect();
    let mut accumulators = lock(&SCROLL);
    accumulators.retain(|key, _| live.contains(key));
    let actions = accumulators.entry(item.clone()).or_default().push(scroll);
    drop(accumulators);
    for action in actions {
        submit(item.clone(), action, anchor.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mango::OutputGeometry;
    use crate::tray::backend::tests::TestBus;
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::{Duration, Instant};

    struct Fixture {
        calls: Arc<Mutex<Vec<(String, i32, i32)>>>,
    }
    #[zbus::interface(name = "org.kde.StatusNotifierItem")]
    impl Fixture {
        fn activate(&self, x: i32, y: i32) {
            self.calls.lock().unwrap().push(("Activate".into(), x, y));
        }
        fn secondary_activate(&self, x: i32, y: i32) {
            self.calls
                .lock()
                .unwrap()
                .push(("SecondaryActivate".into(), x, y));
        }
        fn scroll(&self, delta: i32, orientation: &str) {
            self.calls
                .lock()
                .unwrap()
                .push((orientation.into(), delta, 0));
        }
    }

    #[test]
    fn item_calls_use_captured_output_and_generation() {
        let bus = TestBus::new();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let server = zbus::blocking::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at(
                "/Item",
                Fixture {
                    calls: calls.clone(),
                },
            )
            .unwrap()
            .build()
            .unwrap();
        let client = bus.connect();
        let key = ItemKey {
            owner: server.unique_name().unwrap().to_string(),
            path: "/Item".into(),
            generation: 1,
        };
        let anchor = SlotAnchor {
            output: "right".into(),
            x: 10.0,
            y: 7.0,
            width: 26.0,
            height: 26.0,
        };
        let outputs = [OutputGeometry {
            name: "right".into(),
            x: 1920,
            y: -20,
            width: 1920,
            height: 1080,
            scale: 1.25,
        }];
        execute(
            &client,
            &key,
            ItemAction::Activate,
            &anchor,
            &outputs,
            || true,
        )
        .unwrap();
        execute(
            &client,
            &key,
            ItemAction::SecondaryActivate,
            &anchor,
            &outputs,
            || true,
        )
        .unwrap();
        execute(&client, &key, ItemAction::Activate, &anchor, &[], || true).unwrap();
        execute(
            &client,
            &key,
            ItemAction::Activate,
            &anchor,
            &outputs,
            || false,
        )
        .unwrap();
        assert_eq!(
            &*calls.lock().unwrap(),
            &[
                ("Activate".into(), 1943, 0),
                ("SecondaryActivate".into(), 1943, 0)
            ]
        );
    }

    #[test]
    fn scroll_axes_and_fractional_steps() {
        let mut wheel = ScrollAccumulator::default();
        for _ in 0..3 {
            assert!(wheel.push(amane::Scroll { x: 0.0, y: 0.25 }).is_empty());
        }
        assert_eq!(
            wheel.push(amane::Scroll { x: 0.0, y: 0.25 }),
            [ItemAction::Scroll {
                delta: 120,
                axis: ScrollAxis::Vertical
            }]
        );
        assert_eq!(
            wheel.push(amane::Scroll { x: -1.0, y: 0.0 }),
            [ItemAction::Scroll {
                delta: -120,
                axis: ScrollAxis::Horizontal
            }]
        );
        assert!(
            wheel
                .push(amane::Scroll {
                    x: f32::NAN,
                    y: 0.0
                })
                .is_empty()
        );
    }

    #[test]
    fn stalled_action_worker_bounds_queue_and_preserves_order() {
        let queue = ActionQueue::new();
        let (ready_send, ready) = mpsc::channel();
        let (release, release_receive) = mpsc::channel();
        assert!(queue.enqueue(Box::new(move || {
            ready_send.send(()).unwrap();
            release_receive.recv().unwrap();
        })));
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        let order = Arc::new(Mutex::new(Vec::new()));
        for n in 0..64 {
            let order = order.clone();
            assert!(queue.enqueue(Box::new(move || order.lock().unwrap().push(n))));
        }
        assert!(!queue.enqueue(Box::new(|| {})));
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while order.lock().unwrap().len() < 64 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(&*order.lock().unwrap(), &(0..64).collect::<Vec<_>>());
    }
}
