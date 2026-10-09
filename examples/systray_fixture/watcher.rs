use super::{Log, record};
use serde_json::json;
use std::{
    io::BufRead,
    sync::{Arc, Mutex},
};
use zbus::{message::Header, object_server::SignalEmitter};
struct Watcher {
    items: Arc<Mutex<Vec<String>>>,
    log: Log,
}
#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    async fn register_status_notifier_item(
        &self,
        service: &str,
        #[zbus(header)] header: Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let id = if service.starts_with('/') {
            format!(
                "{}{service}",
                header.sender().map(|s| s.as_str()).unwrap_or("")
            )
        } else {
            service.into()
        };
        let added = {
            let mut items = self
                .items
                .lock()
                .map_err(|_| zbus::fdo::Error::Failed("poisoned watcher".into()))?;
            if items.contains(&id) {
                false
            } else {
                items.push(id.clone());
                true
            }
        };
        if added {
            Self::status_notifier_item_registered(&emitter, &id).await?;
        }
        Ok(())
    }
    fn register_status_notifier_host(&self, service: &str) {
        record(&self.log, "watcher", "Host", json!([service]), 0);
    }
    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.lock().map(|i| i.clone()).unwrap_or_default()
    }
    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
    #[zbus(signal)]
    async fn status_notifier_item_registered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;
}
pub(crate) fn run(
    connection: zbus::blocking::Connection,
    log: Log,
) -> Result<(), Box<dyn std::error::Error>> {
    connection.object_server().at(
        "/StatusNotifierWatcher",
        Watcher {
            items: Arc::new(Mutex::new(Vec::new())),
            log: log.clone(),
        },
    )?;
    // The alternate name points at the KDE interface, as deployed watchers commonly do.
    connection.request_name("org.kde.StatusNotifierWatcher")?;
    record(&log, "watcher", "Ready", json!([]), 0);
    for line in std::io::stdin().lock().lines() {
        if serde_json::from_str::<serde_json::Value>(&line?)?["op"] == "quit" {
            break;
        }
    }
    Ok(())
}
