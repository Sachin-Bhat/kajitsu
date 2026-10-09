use std::io::{BufRead, BufReader};
use std::sync::LazyLock;
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread;
use std::time::Duration;

use amane::Service;
use serde_json::Value;

#[derive(Default)]
pub struct Layouts {
    available: Vec<String>,
    monitors: Vec<MonitorLayout>,
}

impl Service for Layouts {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {
        let initial = Duration::from_millis(250);
        let mut delay = initial;
        loop {
            let connection = super::request("get layouts")
                .and_then(|reply| catalog(&reply))
                .and_then(|available| {
                    super::connect("watch all-monitors").map(|stream| (available, stream))
                });
            if let Ok((available, stream)) = connection {
                for line in BufReader::new(stream).lines() {
                    let Ok(states) = line
                        .map_err(|error| error.to_string())
                        .and_then(|reply| monitors(&reply))
                    else {
                        break;
                    };
                    delay = initial;
                    Self::replace(&available, states);
                }
            }
            Self::replace(&[], Vec::new());
            thread::sleep(delay);
            delay = (delay * 2).min(Duration::from_secs(5));
        }
    }
}

impl Layouts {
    pub fn current(&self, output: &str) -> Option<&str> {
        let monitor = self
            .monitors
            .iter()
            .find(|monitor| monitor.output == output)?;
        self.available.get(monitor.index).map(String::as_str)
    }

    fn replace(available: &[String], monitors: Vec<MonitorLayout>) {
        let changed = {
            let state = Self::read();
            state.available != available || state.monitors != monitors
        };
        if changed {
            let mut state = Self::write();
            state.available = available.to_vec();
            state.monitors = monitors;
        }
    }

    pub fn cycle(output: String, steps: i32) {
        if steps != 0
            && let Err(TrySendError::Disconnected(_)) = ACTIONS.try_send((output, steps))
        {
            eprintln!("kajitsu: layout control worker stopped");
        }
    }
}

// Serialized, bounded control calls keep IPC waits and touchpad bursts off the drawing thread.
static ACTIONS: LazyLock<SyncSender<(String, i32)>> = LazyLock::new(|| {
    let (sender, receiver) = mpsc::sync_channel::<(String, i32)>(8);
    thread::spawn(move || {
        for (output, steps) in receiver {
            if let Err(error) = cycle(&output, steps, super::request) {
                eprintln!("kajitsu: cannot switch layout on {output}: {error}");
            }
        }
    });
    sender
});

#[derive(Debug, Clone, PartialEq, Eq)]
struct MonitorLayout {
    output: String,
    focused: bool,
    index: usize,
}

fn catalog(reply: &str) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_str(reply).map_err(|error| error.to_string())?;
    let items = value["layouts"]
        .as_array()
        .filter(|items| !items.is_empty())
        .ok_or("Mango has no available layout list")?;
    items
        .iter()
        .map(|item| {
            let name = item["name"]
                .as_str()
                .filter(|name| {
                    !name.is_empty()
                        && name
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                })
                .ok_or("Mango layout has an invalid name")?;
            Ok(name.to_string())
        })
        .collect()
}

fn monitors(reply: &str) -> Result<Vec<MonitorLayout>, String> {
    let value: Value = serde_json::from_str(reply).map_err(|error| error.to_string())?;
    value["monitors"]
        .as_array()
        .ok_or("Mango has no monitor list")?
        .iter()
        .map(|monitor| {
            let output = monitor["name"]
                .as_str()
                .filter(|name| !name.is_empty())
                .ok_or("Mango monitor has no name")?;
            let focused = monitor["active"]
                .as_bool()
                .ok_or("Mango monitor has no focus state")?;
            let index = monitor["layout_index"]
                .as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .ok_or("Mango monitor has no valid layout index")?;
            Ok(MonitorLayout {
                output: output.to_string(),
                focused,
                index,
            })
        })
        .collect()
}

fn cycle(
    output: &str,
    steps: i32,
    mut request: impl FnMut(&str) -> Result<String, String>,
) -> Result<(), String> {
    if output.is_empty() || output.contains([',', ':', '\n', '\r', '\0']) {
        return Err("Invalid Mango output selector".into());
    }
    let available = catalog(&request("get layouts")?)?;
    let mut states = monitors(&request("get all-monitors")?)?;
    let mut monitor = states
        .iter()
        .find(|monitor| monitor.output == output)
        .ok_or("Layout output is no longer available")?;
    if monitor.index >= available.len() {
        return Err("Current layout is outside Mango's layout list".into());
    }
    if !monitor.focused {
        let mut selector = String::from("^");
        for character in output.chars() {
            if r"\.^$|?*+()[]{}".contains(character) {
                selector.push('\\');
            }
            selector.push(character);
        }
        selector.push('$');
        super::check_success(&request(&format!("dispatch focusmon,{selector}"))?)?;
        states = monitors(&request("get all-monitors")?)?;
        monitor = states
            .iter()
            .find(|monitor| monitor.output == output && monitor.focused)
            .ok_or("Mango did not focus the layout output")?;
    }
    if monitor.index >= available.len() {
        return Err("Current layout is outside Mango's layout list".into());
    }
    let index =
        (monitor.index as i64 + i64::from(steps)).rem_euclid(available.len() as i64) as usize;
    // Mango 0.17.5 targets the focused monitor; separate IPC calls cannot prevent
    // focus changing between the snapshot above and this dispatch.
    super::check_success(&request(&format!(
        "dispatch setlayout,{}",
        available[index]
    ))?)
}

#[derive(Default)]
pub struct ScrollSteps(f32);

impl ScrollSteps {
    pub fn push(&mut self, lines: f32) -> i32 {
        if !lines.is_finite() {
            return 0;
        }
        let total = (self.0 + lines).clamp(-4.0, 4.0);
        let steps = total.trunc() as i32;
        self.0 = total - steps as f32;
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG: &str = r#"{"layouts":[
        {"symbol":"T","name":"tile"},
        {"symbol":"S","name":"scroller"},
        {"symbol":"G","name":"grid"}
    ]}"#;
    const MONITORS: &str = r#"{"monitors":[
        {"name":"DP-1","active":false,"layout_index":1,"layout_symbol":"S","active_tags":[1,3]},
        {"name":"DP-10","active":true,"layout_index":0,"layout_symbol":"T","active_tags":[1]}
    ]}"#;

    #[test]
    fn reads_each_monitors_layout_instead_of_using_the_focused_output() {
        let layouts = catalog(CATALOG).unwrap();
        let states = monitors(MONITORS).unwrap();
        assert_eq!(layouts[states[0].index], "scroller");
        assert_eq!(layouts[states[1].index], "tile");
        assert!(!states[0].focused);
        assert!(states[1].focused);
    }

    #[test]
    fn wraps_both_directions_using_fresh_layout_state() {
        for (index, steps, expected) in [(2, 1, "tile"), (0, -1, "grid"), (1, 2, "tile")] {
            let reply = format!(
                r#"{{"monitors":[{{"name":"DP-1","active":true,"layout_index":{index}}}]}}"#
            );
            let mut commands = Vec::new();
            cycle("DP-1", steps, |command| {
                commands.push(command.to_string());
                Ok(match command {
                    "get layouts" => CATALOG.to_string(),
                    "get all-monitors" => reply.clone(),
                    _ => r#"{"success":true}"#.to_string(),
                })
            })
            .unwrap();
            assert_eq!(
                commands.last().unwrap(),
                &format!("dispatch setlayout,{expected}")
            );
        }
    }

    #[test]
    fn focuses_the_clicked_monitor_with_an_exact_selector_before_switching() {
        let mut commands = Vec::new();
        let mut reads = 0;
        cycle("DP-1", 1, |command| {
            commands.push(command.to_string());
            Ok(match command {
                "get layouts" => CATALOG.to_string(),
                "get all-monitors" => {
                    reads += 1;
                    if reads == 1 {
                        MONITORS.to_string()
                    } else {
                        MONITORS
                            .replace("\"active\":false", "\"active\":true")
                            .replacen(
                                "\"active\":true,\"layout_index\":0",
                                "\"active\":false,\"layout_index\":0",
                                1,
                            )
                    }
                }
                _ => r#"{"success":true}"#.to_string(),
            })
        })
        .unwrap();
        assert_eq!(
            commands,
            [
                "get layouts",
                "get all-monitors",
                "dispatch focusmon,^DP-1$",
                "get all-monitors",
                "dispatch setlayout,grid"
            ]
        );
    }

    #[test]
    fn never_switches_another_output_after_focus_fails_or_is_not_confirmed() {
        for focus_reply in [
            r#"{"success":false,"error":"disabled output"}"#,
            r#"{"success":true}"#,
        ] {
            let mut commands = Vec::new();
            let result = cycle("DP-1", 1, |command| {
                commands.push(command.to_string());
                Ok(match command {
                    "get layouts" => CATALOG.to_string(),
                    "get all-monitors" => MONITORS.to_string(),
                    _ => focus_reply.to_string(),
                })
            });
            assert!(result.is_err());
            assert!(
                !commands
                    .iter()
                    .any(|command| command.starts_with("dispatch setlayout"))
            );
        }
    }

    #[test]
    fn rejects_removed_outputs_and_layout_indices_outside_the_catalog() {
        for (output, reply) in [
            ("gone", MONITORS.to_string()),
            (
                "DP-1",
                MONITORS.replace("\"layout_index\":1", "\"layout_index\":99"),
            ),
        ] {
            let mut commands = Vec::new();
            assert!(
                cycle(output, 1, |command| {
                    commands.push(command.to_string());
                    Ok(if command == "get layouts" {
                        CATALOG.to_string()
                    } else {
                        reply.clone()
                    })
                })
                .is_err()
            );
            assert!(
                !commands
                    .iter()
                    .any(|command| command.starts_with("dispatch"))
            );
        }
    }

    #[test]
    fn rejects_malformed_state_and_names_that_can_change_dispatch_arguments() {
        for reply in [
            "not json",
            r#"{"layouts":[]}"#,
            r#"{"layouts":[{"name":"tile,quit"}]}"#,
            r#"{"layouts":[{"name":"tile\ndispatch quit"}]}"#,
        ] {
            assert!(catalog(reply).is_err());
        }
        for reply in [
            "not json",
            r#"{"error":"unavailable"}"#,
            r#"{"monitors":[{"name":"DP-1","active":true}]}"#,
        ] {
            assert!(monitors(reply).is_err());
        }
        for name in ["DP-1,2", "DP-1\ndispatch quit", "DP-1:2"] {
            let mut dispatched = false;
            assert!(
                cycle(name, 1, |_| {
                    dispatched = true;
                    Ok(String::new())
                })
                .is_err()
            );
            assert!(!dispatched);
        }
    }

    #[test]
    fn accumulates_fractional_scroll_without_skipping_layouts_for_every_pixel() {
        let mut scroll = ScrollSteps::default();
        assert_eq!(scroll.push(0.25), 0);
        assert_eq!(scroll.push(0.5), 0);
        assert_eq!(scroll.push(0.25), 1);
        assert_eq!(scroll.push(-0.5), 0);
        assert_eq!(scroll.push(-0.5), -1);
        assert_eq!(scroll.push(f32::NAN), 0);
        assert_eq!(scroll.push(1.0), 1);
    }
}
