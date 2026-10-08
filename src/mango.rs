use amane::Workspace;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub fn desktop_empty(tags: &[TagState]) -> bool {
    let selected: Vec<_> = tags.iter().filter(|tag| tag.selected).collect();
    !selected.is_empty() && selected.iter().all(|tag| tag.windows == 0 && !tag.global)
}

pub fn widgets_visible(mode: &str, tags: &[TagState]) -> bool {
    match mode {
        "always" => true,
        "hidden" => false,
        _ => desktop_empty(tags),
    }
}

pub fn active_label(tags: &[TagState]) -> String {
    let selected: Vec<_> = tags
        .iter()
        .filter(|tag| tag.selected)
        .map(|tag| tag.index.to_string())
        .collect();
    match selected.len() {
        0 => "Tags unavailable".into(),
        1 => format!("Tag {}", selected[0]),
        _ => format!("Tags {}", selected.join(", ")),
    }
}

pub fn focused_output() -> Result<String, String> {
    parse_focused_output(&request("get all-monitors")?)
}

pub fn quit() -> Result<(), String> {
    check_success(&request("dispatch quit")?)
}

fn request(command: &str) -> Result<String, String> {
    let path =
        std::env::var_os("MANGO_INSTANCE_SIGNATURE").ok_or("Mango IPC socket is unavailable")?;
    let mut stream =
        UnixStream::connect(path).map_err(|e| format!("Cannot connect to Mango: {e}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    writeln!(stream, "{command}").map_err(|e| format!("Mango request failed: {e}"))?;
    read_reply(stream, Duration::from_secs(3))
}

fn read_reply(stream: UnixStream, timeout: Duration) -> Result<String, String> {
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(stream.take(1024 * 1024))
        .read_line(&mut reply)
        .map_err(|e| format!("Mango reply failed: {e}"))?;
    if !reply.ends_with('\n') {
        return Err("Mango reply was incomplete".into());
    }
    Ok(reply)
}

fn parse_focused_output(reply: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(reply).map_err(|e| format!("Invalid Mango reply: {e}"))?;
    let monitors = value["monitors"]
        .as_array()
        .ok_or("Mango reply has no monitor list")?;
    let active: Vec<_> = monitors
        .iter()
        .filter(|monitor| monitor["active"].as_bool() == Some(true))
        .collect();
    if active.len() != 1 {
        return Err("Mango has no unambiguous focused output".into());
    }
    let name = active[0]["name"]
        .as_str()
        .filter(|name| !name.is_empty())
        .ok_or("Focused output has no name")?;
    Ok(name.into())
}

fn check_success(reply: &str) -> Result<(), String> {
    let value: serde_json::Value =
        serde_json::from_str(reply).map_err(|e| format!("Invalid Mango reply: {e}"))?;
    if value["success"].as_bool() == Some(true) {
        return Ok(());
    }
    Err(format!(
        "Mango action failed: {}",
        value["error"].as_str().unwrap_or("missing success reply")
    ))
}

#[derive(Debug, Clone, Copy)]
pub struct TagState {
    pub index: u32,
    pub selected: bool,
    pub windows: u32,
    pub global: bool,
}

pub fn tag_states(workspaces: &[Workspace], output: &str) -> Vec<TagState> {
    let mut tags: Vec<_> = workspaces
        .iter()
        .filter(|w| w.output() == Some(output))
        .map(|w| TagState {
            index: w.index(),
            selected: w.active(),
            windows: w.windows(),
            global: w.visible_global(),
        })
        .collect();
    tags.sort_by_key(|tag| tag.index);
    tags
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;
    #[test]
    fn desktop_requires_every_selected_tag_to_be_empty() {
        let mut tags = [
            TagState {
                index: 1,
                selected: true,
                windows: 0,
                global: false,
            },
            TagState {
                index: 3,
                selected: true,
                windows: 1,
                global: false,
            },
        ];
        assert!(!desktop_empty(&tags));
        tags[1].windows = 0;
        assert!(desktop_empty(&tags));
        tags[1].global = true;
        assert!(!desktop_empty(&tags));
        tags[1].global = false;
        tags[0].selected = false;
        tags[1].selected = false;
        assert!(!desktop_empty(&tags));
        assert!(!desktop_empty(&[]));
        assert!(widgets_visible("always", &[]));
        assert!(!widgets_visible("hidden", &tags));
    }
    #[test]
    fn labels_all_selected_tags_and_ignores_unselected_tags() {
        let tags = [
            TagState {
                index: 1,
                selected: true,
                windows: 0,
                global: false,
            },
            TagState {
                index: 2,
                selected: false,
                windows: 0,
                global: false,
            },
            TagState {
                index: 3,
                selected: true,
                windows: 1,
                global: false,
            },
        ];
        assert_eq!(active_label(&tags), "Tags 1, 3");
        assert_eq!(active_label(&tags[..1]), "Tag 1");
        assert_eq!(active_label(&[]), "Tags unavailable");
    }
    #[test]
    fn fresh_output_reply_rejects_unavailable_removed_or_malformed_outputs() {
        for name in ["DP-9", "eDP-1"] {
            assert_eq!(
                parse_focused_output(&format!(
                    r#"{{"monitors":[{{"name":"{name}","active":true}}]}}"#
                ))
                .unwrap(),
                name
            );
        }
        for reply in [
            "not json",
            r#"{"monitors":[]}"#,
            r#"{"monitors":[{"name":"DP-9","active":false}]}"#,
            r#"{"monitors":[{"name":"","active":true}]}"#,
        ] {
            assert!(parse_focused_output(reply).is_err());
        }
    }
    #[test]
    fn bounds_stalled_replies_and_checks_dispatch_success() {
        let (_server, client) = UnixStream::pair().unwrap();
        assert!(read_reply(client, Duration::from_millis(30)).is_err());
        assert!(check_success(r#"{"success":true}"#).is_ok());
        assert!(check_success(r#"{"success":false,"error":"refused"}"#).is_err());
        assert!(check_success(r#"{"error":"unknown command"}"#).is_err());
    }
}
