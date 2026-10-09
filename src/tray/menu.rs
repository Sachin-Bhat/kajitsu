pub(crate) mod client;

use super::model::{IconSources, IconSpec, ItemIcon};
use std::collections::{BTreeMap, HashMap};
use zbus::zvariant::OwnedValue;

pub(crate) type RawMenuNode = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MenuKind {
    Entry,
    Separator,
    Check { state: i32 },
    Radio { state: i32 },
}

#[derive(Clone)]
pub(crate) struct MenuNode {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub visible: bool,
    pub kind: MenuKind,
    pub icon: ItemIcon,
    pub children: Vec<i32>,
    pub has_submenu: bool,
}

#[derive(Clone)]
pub(crate) struct MenuTree {
    pub revision: u32,
    pub root: i32,
    pub nodes: BTreeMap<i32, MenuNode>,
}

#[derive(Debug)]
pub(crate) enum MenuError {
    Malformed,
    DuplicateId,
    TooManyNodes,
    TooDeep,
}

pub(crate) fn display_label(label: &str) -> String {
    let mut chars = label.chars().peekable();
    let mut result = String::new();
    while let Some(ch) = chars.next() {
        if ch != '_' {
            result.push(ch);
        } else if chars.peek() == Some(&'_') {
            result.push('_');
            chars.next();
        }
    }
    result
}

pub(crate) fn decode_layout(revision: u32, root: RawMenuNode) -> Result<MenuTree, MenuError> {
    let root_id = root.0;
    let mut nodes = BTreeMap::new();
    decode(root, 1, &mut nodes)?;
    Ok(MenuTree {
        revision,
        root: root_id,
        nodes,
    })
}

fn decode(
    raw: RawMenuNode,
    depth: usize,
    nodes: &mut BTreeMap<i32, MenuNode>,
) -> Result<(), MenuError> {
    if depth > 16 {
        return Err(MenuError::TooDeep);
    }
    if nodes.len() >= 512 {
        return Err(MenuError::TooManyNodes);
    }
    let (id, properties, children) = raw;
    if nodes.contains_key(&id) {
        return Err(MenuError::DuplicateId);
    }
    let string = |name: &str| {
        properties
            .get(name)
            .and_then(|v| <&str>::try_from(v).ok())
            .unwrap_or("")
    };
    let flag = |name: &str| {
        properties
            .get(name)
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(true)
    };
    let state = properties
        .get("toggle-state")
        .and_then(|v| i32::try_from(v).ok())
        .unwrap_or(0);
    let kind = if string("type") == "separator" {
        MenuKind::Separator
    } else {
        match string("toggle-type") {
            "checkmark" => MenuKind::Check { state },
            "radio" => MenuKind::Radio { state },
            _ => MenuKind::Entry,
        }
    };
    let mut icon = ItemIcon::default();
    if !string("icon-name").is_empty() {
        icon = super::icons::resolve(
            &IconSources {
                normal: IconSpec {
                    name: string("icon-name").into(),
                    ..Default::default()
                },
                ..Default::default()
            },
            24,
        );
    }
    if icon.normal.is_none()
        && let Some(data) = properties
            .get("icon-data")
            .and_then(|v| Vec::<u8>::try_from(v.try_clone().ok()?).ok())
    {
        icon.normal = super::icons::read_menu_icon(&data);
    }
    let children: Vec<RawMenuNode> = children
        .into_iter()
        .map(|child| child.try_into().map_err(|_| MenuError::Malformed))
        .collect::<Result<_, _>>()?;
    let has_submenu = string("children-display") == "submenu" || !children.is_empty();
    nodes.insert(
        id,
        MenuNode {
            id,
            label: display_label(string("label")),
            enabled: flag("enabled"),
            visible: flag("visible"),
            kind,
            icon,
            children: children.iter().map(|c| c.0).collect(),
            has_submenu,
        },
    );
    for child in children {
        decode(child, depth + 1, nodes)?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use zbus::zvariant::{OwnedValue, Str, Value};

    pub(crate) fn node(id: i32, label: &str, children: Vec<RawMenuNode>) -> RawMenuNode {
        let properties = HashMap::from([("label".into(), OwnedValue::from(Str::from(label)))]);
        (
            id,
            properties,
            children
                .into_iter()
                .map(|child| OwnedValue::try_from(Value::from(child)).unwrap())
                .collect(),
        )
    }

    #[test]
    fn menu_decode_bounds_and_mnemonics() {
        assert_eq!(display_label("_Open __folder"), "Open _folder");
        let flat = |count| {
            node(
                0,
                "",
                (1..count).map(|id| node(id, "entry", vec![])).collect(),
            )
        };
        assert_eq!(decode_layout(1, flat(512)).unwrap().nodes.len(), 512);
        assert!(decode_layout(1, flat(513)).is_err());
        let deep = |count| {
            (1..count).fold(node(0, "leaf", vec![]), |child, id| {
                node(id, "parent", vec![child])
            })
        };
        assert!(decode_layout(1, deep(16)).is_ok());
        assert!(decode_layout(1, deep(17)).is_err());
        assert!(
            decode_layout(
                1,
                node(0, "", vec![node(1, "A", vec![]), node(1, "B", vec![])])
            )
            .is_err()
        );
        assert!(decode_layout(1, (0, HashMap::new(), vec![OwnedValue::from(7_i32)])).is_err());
        let mut check = node(1, "_Checked", vec![]);
        check.1.insert(
            "toggle-type".into(),
            OwnedValue::from(Str::from("checkmark")),
        );
        check
            .1
            .insert("toggle-state".into(), OwnedValue::from(1_i32));
        let tree = decode_layout(7, node(0, "", vec![check])).unwrap();
        assert_eq!(tree.nodes[&1].kind, MenuKind::Check { state: 1 });
        assert_eq!(tree.nodes[&1].label, "Checked");
    }
}
