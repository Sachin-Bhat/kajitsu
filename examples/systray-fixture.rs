//! Controlled StatusNotifier fixture. Writes only its log and a sibling SVG icon.
#[path = "systray_fixture/menu.rs"]
mod menu;
#[path = "systray_fixture/watcher.rs"]
mod watcher;

use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use zbus::{
    blocking::{Connection, Proxy},
    zvariant::OwnedObjectPath,
};

type Pixels = Vec<(i32, i32, Vec<u8>)>;
pub(crate) type Log = Arc<Mutex<File>>;
pub(crate) fn record(log: &Log, item: &str, method: &str, args: Value, generation: u32) {
    if let Ok(mut file) = log.lock() {
        let _ = writeln!(
            file,
            "{}",
            json!({"item":item,"method":method,"args":args,"generation":generation})
        );
        let _ = file.flush();
    }
}
#[derive(Default)]
pub(crate) struct State {
    status: String,
    generation: u32,
    menu_version: u32,
    malformed: bool,
    stall_ms: u64,
    long_menu: bool,
}
pub(crate) struct Item {
    id: String,
    menu: String,
    icon: String,
    state: Arc<Mutex<State>>,
    log: Log,
}
impl Item {
    fn call(&self, method: &str, args: Value) {
        if let Ok(state) = self.state.lock() {
            record(&self.log, &self.id, method, args, state.generation);
            std::thread::sleep(Duration::from_millis(state.stall_ms));
        }
    }
}
fn pixels(size: i32, rgb: [u8; 3]) -> Pixels {
    vec![(
        size,
        size,
        (0..size * size)
            .flat_map(|_| [255, rgb[0], rgb[1], rgb[2]])
            .collect(),
    )]
}
#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    fn activate(&self, x: i32, y: i32) {
        self.call("Activate", json!([x, y]));
    }
    fn context_menu(&self, x: i32, y: i32) {
        self.call("ContextMenu", json!([x, y]));
    }
    fn secondary_activate(&self, x: i32, y: i32) {
        self.call("SecondaryActivate", json!([x, y]));
    }
    fn scroll(&self, delta: i32, axis: &str) {
        self.call("Scroll", json!([delta, axis]));
    }
    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }
    #[zbus(property)]
    fn id(&self) -> &str {
        &self.id
    }
    #[zbus(property)]
    fn title(&self) -> String {
        format!("Fixture {} · 設定", self.id)
    }
    #[zbus(property)]
    fn status(&self) -> String {
        self.state
            .lock()
            .map(|s| s.status.clone())
            .unwrap_or_else(|_| "Passive".into())
    }
    #[zbus(property)]
    fn icon_name(&self) -> &str {
        if self.id == "named" { &self.icon } else { "" }
    }
    #[zbus(property)]
    fn icon_pixmap(&self) -> Pixels {
        let Ok(s) = self.state.lock() else {
            return vec![];
        };
        if s.generation == 999 {
            return vec![(24, 24, vec![1])];
        }
        pixels(
            24,
            if s.generation % 2 == 0 {
                [40, 200, 200]
            } else {
                [220, 60, 130]
            },
        )
    }
    #[zbus(property)]
    fn attention_icon_pixmap(&self) -> Pixels {
        pixels(24, [245, 165, 30])
    }
    #[zbus(property)]
    fn overlay_icon_pixmap(&self) -> Pixels {
        if self.id == "attention" {
            pixels(8, [80, 220, 70])
        } else {
            vec![]
        }
    }
    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        self.id == "menu"
    }
    #[zbus(property)]
    fn menu(&self) -> zbus::fdo::Result<OwnedObjectPath> {
        OwnedObjectPath::try_from(self.menu.clone())
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }
    #[zbus(property)]
    fn tool_tip(&self) -> (String, Pixels, String, String) {
        (
            String::new(),
            vec![],
            format!("Tooltip {}", self.id),
            "<b>Plain &amp; safe</b><script>omit me</script> · café".into(),
        )
    }
}

fn register(connection: &Connection, advertised: &str) -> zbus::Result<()> {
    for interface in [
        "org.kde.StatusNotifierWatcher",
        "org.freedesktop.StatusNotifierWatcher",
    ] {
        Proxy::new(connection, interface, "/StatusNotifierWatcher", interface)?
            .call::<_, _, ()>("RegisterStatusNotifierItem", &advertised)?;
    }
    Ok(())
}
fn add(
    connection: &Connection,
    states: &mut HashMap<String, (String, Arc<Mutex<State>>)>,
    id: &str,
    icon: &str,
    log: &Log,
) -> Result<(), Box<dyn std::error::Error>> {
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("invalid fixture ID".into());
    }
    if states.contains_key(id) {
        return Ok(());
    }
    let path = if id == "named" {
        "/StatusNotifierItem".into()
    } else {
        format!("/Items/{id}")
    };
    let menu_path = if id == "raw" {
        "/".into()
    } else {
        format!("/Menus/{id}")
    };
    let state = Arc::new(Mutex::new(State {
        status: if id == "attention" {
            "NeedsAttention"
        } else {
            "Active"
        }
        .into(),
        ..Default::default()
    }));
    connection.object_server().at(
        path.as_str(),
        Item {
            id: id.into(),
            menu: menu_path.clone(),
            icon: icon.into(),
            state: state.clone(),
            log: log.clone(),
        },
    )?;
    if menu_path != "/" {
        connection.object_server().at(
            menu_path.as_str(),
            menu::Menu {
                id: id.into(),
                icon: icon.into(),
                state: state.clone(),
                log: log.clone(),
            },
        )?;
    }
    register(
        connection,
        if id == "named" {
            "org.kde.StatusNotifierItem.kajitsuFixture"
        } else {
            &path
        },
    )?;
    register(connection, &path)?;
    record(log, id, "Registered", json!([path]), 0);
    states.insert(id.into(), (path, state));
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let option = |key: &str| {
        args.windows(2)
            .find(|pair| pair[0] == key)
            .map(|pair| pair[1].clone())
    };
    let log_path = PathBuf::from(option("--log").ok_or("--log is required")?);
    let log = Arc::new(Mutex::new(File::create(&log_path)?));
    let connection = zbus::blocking::connection::Builder::session()?
        .method_timeout(Duration::from_secs(3))
        .build()?;
    if option("--mode").as_deref() == Some("watcher") {
        return watcher::run(connection, log);
    }
    let icon = log_path.with_extension("svg");
    std::fs::write(
        &icon,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\"><rect width=\"24\" height=\"24\" fill=\"#73a9f5\"/><circle cx=\"12\" cy=\"12\" r=\"7\" fill=\"#274272\"/></svg>",
    )?;
    connection.request_name("org.kde.StatusNotifierItem.kajitsuFixture")?;
    let mut states = HashMap::new();
    for id in ["named", "raw", "menu", "attention"] {
        add(&connection, &mut states, id, &icon.to_string_lossy(), &log)?;
    }
    record(
        &log,
        "fixture",
        "Ready",
        json!([connection.unique_name().map(ToString::to_string)]),
        0,
    );
    for line in std::io::stdin().lock().lines() {
        let command: Value = serde_json::from_str(&line?)?;
        let op = command["op"].as_str().unwrap_or("").to_string();
        if op == "quit" {
            break;
        }
        let id = command["id"].as_str().unwrap_or("named").to_string();
        if op == "add" {
            add(&connection, &mut states, &id, &icon.to_string_lossy(), &log)?;
            continue;
        }
        let Some((path, state)) = states.get(&id) else {
            continue;
        };
        let mut s = state.lock().map_err(|_| "fixture state poisoned")?;
        match op.as_str() {
            "status" => s.status = command["value"].as_str().unwrap_or("Active").into(),
            "remove" => s.status = "Passive".into(),
            "icon" => s.generation = command["generation"].as_u64().unwrap_or(1) as u32,
            "menu_update" => s.menu_version += 1,
            "menu_bad" => s.malformed = true,
            "long_menu" => s.long_menu = true,
            "stall" => s.stall_ms = command["milliseconds"].as_u64().unwrap_or(0),
            _ => continue,
        }
        let generation = s.generation;
        drop(s);
        if matches!(op.as_str(), "menu_update" | "menu_bad" | "long_menu") {
            let menu_path = format!("/Menus/{id}");
            connection.emit_signal(
                None::<&str>,
                menu_path.as_str(),
                "com.canonical.dbusmenu",
                "LayoutUpdated",
                &(generation + 1, 0_i32),
            )?;
            connection.emit_signal(
                None::<&str>,
                menu_path.as_str(),
                "com.canonical.dbusmenu",
                "ItemsPropertiesUpdated",
                &(
                    Vec::<(i32, HashMap<String, zbus::zvariant::OwnedValue>)>::new(),
                    Vec::<(i32, Vec<String>)>::new(),
                ),
            )?;
        } else {
            connection.emit_signal(
                None::<&str>,
                path.as_str(),
                "org.kde.StatusNotifierItem",
                "NewIcon",
                &(),
            )?;
        }
        record(&log, &id, "Control", command, generation);
    }
    Ok(())
}
