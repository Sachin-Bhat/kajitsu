use super::{hex, terminal, write_owned};
use crate::theme::Theme;
use std::path::{Path, PathBuf};
use toml_edit::DocumentMut;

pub fn source_path() -> PathBuf {
    PathBuf::from(super::config_home()).join("bottom/bottom.toml")
}

pub(super) fn read_source(source: &Path) -> Result<String, String> {
    match std::fs::read_to_string(source) {
        Ok(base) => Ok(base),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("cannot read {}: {error}", source.display())),
    }
}

pub fn render(base: &str, theme: &Theme) -> Result<String, String> {
    let mut document: DocumentMut = base
        .parse()
        .map_err(|e| format!("invalid bottom config: {e}"))?;
    let colors = terminal::colors(theme);
    let array = |values: &[amane::Color]| {
        values
            .iter()
            .map(|c| format!("\"{}\"", hex(*c)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let styles = format!(
        r#"[styles]
theme = "{mode}"
[styles.cpu]
all_entry_color = "{accent}"
avg_entry_color = "{success}"
cpu_core_colors = [{ansi}]
[styles.memory]
ram_color = "{accent}"
cache_color = "{muted}"
swap_color = "{danger}"
arc_color = "{hover}"
gpu_colors = [{ansi}]
[styles.network]
rx_color = "{success}"
tx_color = "{accent}"
rx_total_color = "{secondary}"
tx_total_color = "{hover}"
[styles.battery]
high_battery_color = "{success}"
medium_battery_color = "{yellow}"
low_battery_color = "{danger}"
[styles.tables]
headers = {{ color = "{accent}", bold = true }}
[styles.graphs]
graph_color = "{border}"
legend_text = {{ color = "{secondary}" }}
[styles.widgets]
bg_color = "{background}"
border_color = "{border}"
selected_border_color = "{accent}"
widget_title = {{ color = "{text}", bold = true }}
text = {{ color = "{text}" }}
selected_text = {{ color = "{on_accent}", bg_color = "{accent}" }}
disabled_text = {{ color = "{muted}" }}
thread_text = {{ color = "{secondary}" }}
"#,
        mode = if theme.light {
            "default-light"
        } else {
            "default"
        },
        accent = hex(theme.accent),
        success = hex(theme.success),
        muted = hex(theme.muted_text),
        danger = hex(theme.danger),
        hover = hex(theme.accent_hover),
        secondary = hex(theme.secondary_text),
        yellow = hex(colors.ansi[11]),
        border = hex(theme.border),
        background = hex(theme.background),
        text = hex(theme.text),
        on_accent = hex(theme.on_accent),
        ansi = array(&colors.ansi[9..15]),
    );
    let mut generated: DocumentMut = styles
        .parse()
        .map_err(|e| format!("invalid generated bottom styles: {e}"))?;
    let mut styles = generated
        .remove("styles")
        .ok_or("generated styles missing")?;
    append_tables(&mut styles);
    document["styles"] = styles;
    Ok(document.to_string())
}

// Positions from a separate document must not collide with the user's tables.
fn append_tables(item: &mut toml_edit::Item) {
    if let Some(table) = item.as_table_mut() {
        table.set_position(None);
        for (_, child) in table.iter_mut() {
            append_tables(child);
        }
    }
}

pub fn export(theme: &Theme) -> Result<(), String> {
    export_from(
        &source_path(),
        &crate::paths::state_dir().join("bottom.toml"),
        theme,
    )
}

fn export_from(source: &Path, output: &Path, theme: &Theme) -> Result<(), String> {
    let base = read_source(source)?;
    let rendered = render(&base, theme)?;
    write_owned(output, &rendered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::test_theme;
    use toml_edit::DocumentMut;
    const BASE: &str = r#"# Keep my preferences
[flags]
rate = "2s" # Keep this inline comment
basic = true
[processes]
columns = ["PID", "Name", "CPU%"]
[network.interface_filter]
list = ["lo"]
regex = false
[[row]]
ratio = 2
[[row.child]]
type = "proc"
[styles.widgets]
text = "red"
"#;
    #[test]
    fn source_edits_regenerate_with_an_unchanged_theme() {
        let folder =
            std::env::temp_dir().join(format!("kajitsu-bottom-cache-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let source = folder.join("source.toml");
        let output = folder.join("output.toml");
        let theme = test_theme(false);
        let mut exporter = crate::integrations::Export::default();
        std::fs::write(&source, BASE).unwrap();
        exporter
            .export_bottom("same theme".into(), &source, || {
                export_from(&source, &output, &theme)
            })
            .unwrap();
        std::fs::write(&source, BASE.replace("2s", "4s")).unwrap();
        exporter
            .export_bottom("same theme".into(), &source, || {
                export_from(&source, &output, &theme)
            })
            .unwrap();
        assert!(
            std::fs::read_to_string(&output)
                .unwrap()
                .contains("rate = \"4s\"")
        );
        let previous = std::fs::read_to_string(&output).unwrap();
        std::fs::write(&source, "[invalid").unwrap();
        assert!(
            exporter
                .export_bottom("same theme".into(), &source, || export_from(
                    &source, &output, &theme
                ))
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&output).unwrap(), previous);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn preserves_preferences_layout_filters_and_non_style_comments() {
        for light in [false, true] {
            let rendered = render(BASE, &test_theme(light)).unwrap();
            let parsed: DocumentMut = rendered.parse().unwrap();
            assert_eq!(parsed["flags"]["rate"].as_str(), Some("2s"));
            assert_eq!(parsed["flags"]["basic"].as_bool(), Some(true));
            assert_eq!(parsed["processes"]["columns"].as_array().unwrap().len(), 3);
            assert_eq!(
                parsed["network"]["interface_filter"]["list"]
                    .as_array()
                    .unwrap()
                    .get(0)
                    .unwrap()
                    .as_str(),
                Some("lo")
            );
            assert_eq!(
                parsed["row"].as_array_of_tables().unwrap().get(0).unwrap()["ratio"].as_integer(),
                Some(2)
            );
            assert_eq!(
                parsed["styles"]["widgets"]["bg_color"].as_str(),
                Some("#010203")
            );
            assert!(rendered.contains("# Keep my preferences"));
            assert!(rendered.contains("# Keep this inline comment"));
            assert_eq!(render(&rendered, &test_theme(light)).unwrap(), rendered);
        }
    }
    #[test]
    fn missing_source_uses_defaults_and_invalid_source_keeps_last_valid_output() {
        let folder = std::env::temp_dir().join(format!("kajitsu-bottom-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let source = folder.join("source.toml");
        let output = folder.join("generated.toml");
        export_from(&source, &output, &test_theme(false)).unwrap();
        let previous = std::fs::read_to_string(&output).unwrap();
        std::fs::write(&source, "[invalid").unwrap();
        assert!(export_from(&source, &output, &test_theme(false)).is_err());
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "[invalid");
        assert_eq!(std::fs::read_to_string(&output).unwrap(), previous);
        std::fs::write(&source, BASE).unwrap();
        export_from(&source, &output, &test_theme(false)).unwrap();
        assert!(
            std::fs::read_to_string(&output)
                .unwrap()
                .contains("rate = \"2s\"")
        );
        std::fs::write(&source, BASE.replace("2s", "3s")).unwrap();
        export_from(&source, &output, &test_theme(false)).unwrap();
        assert!(
            std::fs::read_to_string(&output)
                .unwrap()
                .contains("rate = \"3s\"")
        );
        std::fs::remove_dir_all(folder).unwrap();
    }
}
