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

#[derive(Debug, Clone)]
pub(crate) struct OutputGeometry {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}

pub(crate) fn output_geometries() -> Result<Vec<OutputGeometry>, String> {
    parse_output_geometries(&request("get all-monitors")?)
}

fn parse_output_geometries(reply: &str) -> Result<Vec<OutputGeometry>, String> {
    let value: serde_json::Value =
        serde_json::from_str(reply).map_err(|e| format!("Invalid Mango reply: {e}"))?;
    let monitors = value["monitors"]
        .as_array()
        .ok_or("Mango reply has no monitor list")?;
    monitors
        .iter()
        .filter(|monitor| {
            !(monitor["width"].as_u64() == Some(0) && monitor["height"].as_u64() == Some(0))
        })
        .map(|monitor| {
            let name = monitor["name"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("Output has no name")?
                .to_string();
            let coordinate = |key: &str| {
                monitor[key]
                    .as_i64()
                    .and_then(|v| i32::try_from(v).ok())
                    .ok_or_else(|| format!("Output has no valid {key}"))
            };
            let dimension = |key: &str| {
                monitor[key]
                    .as_u64()
                    .and_then(|v| u32::try_from(v).ok())
                    .filter(|v| *v > 0)
                    .ok_or_else(|| format!("Output has no valid {key}"))
            };
            let scale = monitor["scale"]
                .as_f64()
                .filter(|s| s.is_finite() && *s > 0.0 && *s <= f64::from(f32::MAX))
                .ok_or("Output has no valid scale")? as f32;
            Ok(OutputGeometry {
                name,
                x: coordinate("x")?,
                y: coordinate("y")?,
                width: dimension("width")?,
                height: dimension("height")?,
                scale,
            })
        })
        .collect()
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
    fn output_geometries_use_logical_origins_and_reject_missing_dimensions() {
        let outputs = parse_output_geometries(r#"{"monitors":[{"name":"eDP-1","x":1920,"y":-20,"width":2304,"height":1440,"scale":1.25}]}"#).unwrap();
        assert_eq!(
            (outputs[0].x, outputs[0].y, outputs[0].width),
            (1920, -20, 2304)
        );
        assert_eq!(outputs[0].scale, 1.25);
        assert!(parse_output_geometries(r#"{"monitors":[{"name":"gone"}]}"#).is_err());
        assert!(
            parse_output_geometries(r#"{"monitors":[]}"#)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn disabled_output_does_not_invalidate_live_output_geometry() {
        let outputs = parse_output_geometries(r#"{"monitors":[{"name":"live","x":0,"y":0,"width":1280,"height":720,"scale":1},{"name":"off","x":0,"y":0,"width":0,"height":0,"scale":1.25}]}"#).unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].name, "live");
    }
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
