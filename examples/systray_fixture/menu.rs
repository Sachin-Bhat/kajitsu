use super::{Log, State, record};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use zbus::zvariant::{OwnedValue, Str, Value};
type Raw = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);
pub(crate) struct Menu {
    pub id: String,
    pub icon: String,
    pub state: Arc<Mutex<State>>,
    pub log: Log,
}
fn node(id: i32, label: &str) -> Raw {
    (
        id,
        HashMap::from([("label".into(), OwnedValue::from(Str::from(label)))]),
        vec![],
    )
}
fn string(node: &mut Raw, key: &str, value: &str) {
    node.1
        .insert(key.into(), OwnedValue::from(Str::from(value)));
}
#[zbus::interface(name = "com.canonical.dbusmenu")]
impl Menu {
    fn about_to_show(&self, id: i32) -> bool {
        record(&self.log, &self.id, "AboutToShow", json!([id]), 0);
        false
    }
    fn get_layout(
        &self,
        parent: i32,
        _depth: i32,
        _names: Vec<String>,
    ) -> zbus::fdo::Result<(u32, Raw)> {
        let state = self
            .state
            .lock()
            .map_err(|_| zbus::fdo::Error::Failed("poisoned fixture".into()))?;
        record(
            &self.log,
            &self.id,
            "GetLayout",
            json!([parent]),
            state.menu_version,
        );
        if state.malformed {
            return Ok((
                state.menu_version,
                (parent, HashMap::new(), vec![OwnedValue::from(3_i32)]),
            ));
        }
        let mut rows = if parent == 6 {
            vec![node(60, "_Child"), node(61, "Other child")]
        } else {
            let mut disabled = node(2, "Disabled");
            disabled.1.insert("enabled".into(), OwnedValue::from(false));
            let mut separator = node(3, "");
            string(&mut separator, "type", "separator");
            let mut check = node(4, "_Checked __literal");
            string(&mut check, "icon-name", &self.icon);
            string(&mut check, "toggle-type", "checkmark");
            check
                .1
                .insert("toggle-state".into(), OwnedValue::from(1_i32));
            let mut radio = node(5, "Radio");
            string(&mut radio, "toggle-type", "radio");
            radio
                .1
                .insert("toggle-state".into(), OwnedValue::from(1_i32));
            let mut submenu = node(6, "_More");
            string(&mut submenu, "children-display", "submenu");
            let mut hidden = node(7, "Hidden");
            hidden.1.insert("visible".into(), OwnedValue::from(false));
            vec![
                node(1, "_Open"),
                disabled,
                separator,
                check,
                radio,
                submenu,
                hidden,
                node(
                    8,
                    "設定 · café · long Unicode label with literal underscores __ and more text",
                ),
            ]
        };
        if state.menu_version > 0 && parent == 0 {
            rows.retain(|row| row.0 != 5);
            if let Some(open) = rows.iter_mut().find(|r| r.0 == 1) {
                open.1.insert("enabled".into(), OwnedValue::from(false));
            }
            rows.rotate_right(1);
        }
        if state.long_menu && parent == 0 {
            rows.extend((101..=140).map(|id| node(id, &format!("Long menu row {id}"))));
        }
        let children = rows
            .into_iter()
            .map(|r| {
                OwnedValue::try_from(Value::from(r))
                    .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
            })
            .collect::<zbus::fdo::Result<Vec<_>>>()?;
        Ok((state.menu_version, (parent, HashMap::new(), children)))
    }
    fn event(&self, id: i32, event: &str, _data: Value<'_>, timestamp: u32) {
        record(
            &self.log,
            &self.id,
            "Event",
            json!([id, event, timestamp]),
            0,
        );
    }
}
