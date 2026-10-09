use std::collections::HashMap;
use std::path::PathBuf;

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{ObjectPath, OwnedValue};

use super::model::{
    IconSources, IconSpec, ItemKey, ItemProperties, ItemStatus, RawPixmap, TooltipText,
};

pub(crate) const INTERFACES: [&str; 2] = [
    "org.kde.StatusNotifierItem",
    "org.freedesktop.StatusNotifierItem",
];

pub(crate) fn read(connection: &Connection, key: &ItemKey) -> zbus::Result<ItemProperties> {
    let proxy = Proxy::new(
        connection,
        key.owner.as_str(),
        key.path.as_str(),
        "org.freedesktop.DBus.Properties",
    )?;
    let mut last = None;
    for interface in INTERFACES {
        match proxy.call::<_, _, HashMap<String, OwnedValue>>("GetAll", &(interface,)) {
            Ok(values) => return Ok(decode(values)),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap_or_else(|| zbus::Error::Failure("Tray item has no supported interface".into())))
}

fn text(values: &HashMap<String, OwnedValue>, key: &str) -> String {
    values
        .get(key)
        .and_then(|value| <&str>::try_from(value).ok())
        .unwrap_or_default()
        .into()
}

fn pixmaps(values: &HashMap<String, OwnedValue>, key: &str) -> Vec<RawPixmap> {
    values
        .get(key)
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| Vec::<RawPixmap>::try_from(value).ok())
        .unwrap_or_default()
}

pub(crate) fn decode(values: HashMap<String, OwnedValue>) -> ItemProperties {
    let id = text(&values, "Id");
    let title = text(&values, "Title");
    let status = match text(&values, "Status").as_str() {
        "Active" | "" => ItemStatus::Active,
        "NeedsAttention" => ItemStatus::NeedsAttention,
        _ => ItemStatus::Passive,
    };
    let tooltip = values
        .get("ToolTip")
        .and_then(|v| v.try_clone().ok())
        .and_then(|v| <(String, Vec<RawPixmap>, String, String)>::try_from(v).ok());
    let tooltip = match tooltip {
        Some((_, _, title, description)) => TooltipText { title, description },
        None => TooltipText {
            title: if title.is_empty() {
                id.clone()
            } else {
                title.clone()
            },
            description: String::new(),
        },
    };
    let theme_path = text(&values, "IconThemePath");
    let icon = |prefix: &str| IconSpec {
        name: text(&values, &format!("{prefix}Name")),
        pixmaps: pixmaps(&values, &format!("{prefix}Pixmap")),
    };
    ItemProperties {
        id,
        title,
        status,
        item_is_menu: values
            .get("ItemIsMenu")
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false),
        menu: values
            .get("Menu")
            .and_then(|v| <&ObjectPath<'_>>::try_from(v).ok())
            .filter(|p| p.as_str() != "/")
            .map(ToString::to_string),
        tooltip,
        icons: IconSources {
            theme_path: (!theme_path.is_empty()).then(|| PathBuf::from(theme_path)),
            normal: icon("Icon"),
            attention: icon("AttentionIcon"),
            overlay: icon("OverlayIcon"),
        },
    }
}

pub(crate) fn plain_text(input: &str) -> String {
    let mut text = String::new();
    let mut rest = input;
    while !rest.is_empty() {
        if rest.starts_with('<') {
            let tag_like = rest
                .chars()
                .nth(1)
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '/' || c == '!');
            if tag_like && let Some(end) = rest.find('>') {
                let tag = rest[1..end].trim().to_ascii_lowercase();
                let name = tag.split_whitespace().next().unwrap_or_default();
                if matches!(
                    name,
                    "script" | "style" | "iframe" | "object" | "svg" | "video" | "audio"
                ) {
                    let lower = rest.to_ascii_lowercase();
                    rest = lower
                        .find(&format!("</{name}"))
                        .and_then(|start| lower[start..].find('>').map(|end| start + end + 1))
                        .map(|end| &rest[end..])
                        .unwrap_or_default();
                    continue;
                }
                if tag.starts_with("br") {
                    text.push('\n');
                }
                rest = &rest[end + 1..];
                continue;
            }
        }
        if rest.starts_with('&')
            && let Some(end) = rest.find(';').filter(|end| *end < 16)
        {
            let entity = &rest[1..end];
            let decoded = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .and_then(|v| u32::from_str_radix(v, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|v| v.parse().ok()))
                    .and_then(char::from_u32),
            };
            if let Some(character) = decoded {
                if !character.is_control() || character == '\n' || character == '\t' {
                    text.push(character);
                }
                rest = &rest[end + 1..];
                continue;
            }
        }
        let Some(character) = rest.chars().next() else {
            break;
        };
        if !character.is_control() || character == '\n' || character == '\t' {
            text.push(character);
        }
        rest = &rest[character.len_utf8()..];
    }
    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::{ObjectPath, Str};

    #[test]
    fn tooltips_strip_markup_and_decode_entities_without_loading_content() {
        assert_eq!(plain_text("<b>Mail</b> &amp; news"), "Mail & news");
        assert_eq!(
            plain_text("<img src='https://example.invalid/a'>Hello &#x1f600;"),
            "Hello 😀"
        );
        assert_eq!(plain_text("3 < 4 &unknown;"), "3 < 4 &unknown;");
        assert_eq!(plain_text("<script>ignored</script><b>Mail</b>"), "Mail");
    }

    #[test]
    fn typed_properties_handle_missing_tooltips_and_invalid_status() {
        let mut values = HashMap::new();
        values.insert("Id".into(), OwnedValue::from(Str::from("mail")));
        values.insert("Title".into(), OwnedValue::from(Str::from("Mail")));
        values.insert(
            "Status".into(),
            OwnedValue::from(Str::from("NeedsAttention")),
        );
        values.insert(
            "Menu".into(),
            OwnedValue::from(ObjectPath::try_from("/").unwrap()),
        );
        let properties = decode(values);
        assert_eq!(properties.status, ItemStatus::NeedsAttention);
        assert_eq!(properties.tooltip.title, "Mail");
        assert!(properties.menu.is_none());
        let values = HashMap::from([("Status".into(), OwnedValue::from(Str::from("unknown")))]);
        assert_eq!(decode(values).status, ItemStatus::Passive);
    }
}
