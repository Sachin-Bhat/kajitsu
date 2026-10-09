use std::sync::Arc;

use zbus::zvariant::{ObjectPath, OwnedValue, Value};
use zbus::{Connection, fdo, message::Header};

use super::backend::{State, WATCHER_PATH, WATCHERS, lock};

async fn owner_of(connection: &Connection, name: &str) -> fdo::Result<String> {
    let bus = zbus::fdo::DBusProxy::new(connection).await?;
    let name =
        zbus::names::BusName::try_from(name).map_err(|e| fdo::Error::InvalidArgs(e.to_string()))?;
    Ok(bus.get_name_owner(name).await?.to_string())
}

async fn register(
    connection: &Connection,
    state: &State,
    service: &str,
    header: Header<'_>,
) -> fdo::Result<()> {
    let (owner, path, advertised) = if service.starts_with('/') {
        ObjectPath::try_from(service).map_err(|e| fdo::Error::InvalidArgs(e.to_string()))?;
        let sender = header
            .sender()
            .ok_or_else(|| fdo::Error::Failed("Registration has no sender".into()))?
            .to_string();
        (
            sender.clone(),
            service.to_string(),
            format!("{sender}{service}"),
        )
    } else {
        let split = service.find('/').unwrap_or(service.len());
        let name = &service[..split];
        let path = if split == service.len() {
            "/StatusNotifierItem"
        } else {
            &service[split..]
        };
        ObjectPath::try_from(path).map_err(|e| fdo::Error::InvalidArgs(e.to_string()))?;
        (
            owner_of(connection, name).await?,
            path.into(),
            format!("{name}{path}"),
        )
    };
    let (_, new) = state.register_owned(owner, path, advertised.clone());
    if new {
        emit_item(connection, "StatusNotifierItemRegistered", &advertised).await?;
    }
    let ids = lock(&state.registry).ids();
    for interface in WATCHERS {
        let value = OwnedValue::try_from(Value::from(ids.clone())).map_err(zbus::Error::from)?;
        let changed = std::collections::HashMap::from([("RegisteredStatusNotifierItems", value)]);
        connection
            .emit_signal(
                None::<&str>,
                WATCHER_PATH,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                &(interface, changed, Vec::<String>::new()),
            )
            .await?;
    }
    Ok(())
}

async fn emit_item(connection: &Connection, member: &str, id: &str) -> zbus::Result<()> {
    let emitter = zbus::object_server::SignalEmitter::new(connection, WATCHER_PATH)?;
    match member {
        "StatusNotifierItemRegistered" => {
            WatcherKde::status_notifier_item_registered(&emitter, id).await?;
            WatcherFreedesktop::status_notifier_item_registered(&emitter, id).await?;
        }
        "StatusNotifierItemUnregistered" => {
            WatcherKde::status_notifier_item_unregistered(&emitter, id).await?;
            WatcherFreedesktop::status_notifier_item_unregistered(&emitter, id).await?;
        }
        _ => return Err(zbus::Error::Failure("Unknown watcher signal".into())),
    }
    Ok(())
}

pub(crate) fn emit_properties_blocking(connection: &zbus::blocking::Connection, ids: Vec<String>) {
    for interface in WATCHERS {
        if let Ok(value) = OwnedValue::try_from(Value::from(ids.clone())) {
            let changed =
                std::collections::HashMap::from([("RegisteredStatusNotifierItems", value)]);
            let _ = connection.emit_signal(
                None::<&str>,
                WATCHER_PATH,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                &(interface, changed, Vec::<String>::new()),
            );
        }
    }
}

pub(crate) fn emit_item_blocking(connection: &zbus::blocking::Connection, member: &str, id: &str) {
    for interface in WATCHERS {
        let _ = connection.emit_signal(None::<&str>, WATCHER_PATH, interface, member, &(id,));
    }
}

macro_rules! watcher {
    ($name:ident, $interface:literal) => {
        pub(crate) struct $name(pub Arc<State>);

        #[zbus::interface(name = $interface)]
        impl $name {
            async fn register_status_notifier_item(
                &self,
                service: &str,
                #[zbus(header)] header: Header<'_>,
                #[zbus(connection)] connection: &Connection,
            ) -> fdo::Result<()> {
                register(connection, &self.0, service, header).await
            }

            async fn register_status_notifier_host(
                &self,
                service: &str,
                #[zbus(connection)] connection: &Connection,
            ) -> fdo::Result<()> {
                let owner = owner_of(connection, service).await?;
                let new = lock(&self.0.hosts).insert(owner);
                if new {
                    let emitter =
                        zbus::object_server::SignalEmitter::new(connection, WATCHER_PATH)?;
                    WatcherKde::status_notifier_host_registered(&emitter).await?;
                    WatcherFreedesktop::status_notifier_host_registered(&emitter).await?;
                }
                Ok(())
            }

            #[zbus(property)]
            fn registered_status_notifier_items(&self) -> Vec<String> {
                lock(&self.0.registry).ids()
            }
            #[zbus(property)]
            fn is_status_notifier_host_registered(&self) -> bool {
                !lock(&self.0.hosts).is_empty()
            }
            #[zbus(property)]
            fn protocol_version(&self) -> i32 {
                0
            }

            #[zbus(signal)]
            async fn status_notifier_item_registered(
                emitter: &zbus::object_server::SignalEmitter<'_>,
                service: &str,
            ) -> zbus::Result<()>;
            #[zbus(signal)]
            async fn status_notifier_item_unregistered(
                emitter: &zbus::object_server::SignalEmitter<'_>,
                service: &str,
            ) -> zbus::Result<()>;
            #[zbus(signal)]
            async fn status_notifier_host_registered(
                emitter: &zbus::object_server::SignalEmitter<'_>,
            ) -> zbus::Result<()>;
        }
    };
}

watcher!(WatcherKde, "org.kde.StatusNotifierWatcher");
watcher!(WatcherFreedesktop, "org.freedesktop.StatusNotifierWatcher");
