mod bottom;
mod cava;
mod gtk;
mod spotify;
mod terminal;
mod tmux;
mod vesktop;
mod wezterm;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use amane::{Color, Service};

use crate::settings::Settings;
use crate::theme::{self, Theme};

// the settings key and the writer of each program that takes the shell's colors
type Program = (&'static str, fn(&Theme));

const PROGRAMS: [Program; 5] = [
    ("integration_gtk", gtk::export),
    ("integration_tmux", tmux::export),
    ("integration_vesktop", vesktop::export),
    ("integration_spotify", spotify::export),
    ("integration_cava", cava::export),
];

/*
 * looks at the theme once a second and writes the colors out to every
 * program turned on in the settings, only when the colors or the programs
 * changed; it runs on its own thread, so drawing never waits on the files
 */
#[derive(Default)]
pub struct Export {
    // the colors and programs written last, so an unchanged theme writes nothing
    written: HashMap<&'static str, String>,
    errors: HashMap<&'static str, String>,
}

impl Service for Export {
    fn new() -> Self {
        Self::default()
    }

    // nothing a window shows changes here
    fn update(&mut self) -> bool {
        let theme = theme::current();

        let settings = Settings::read();

        let enabled: Vec<_> = PROGRAMS
            .iter()
            .filter(|(key, _)| settings.flag(key))
            .copied()
            .collect();
        let wezterm_on = settings.flag("integration_wezterm");
        let bottom_on = settings.flag("integration_bottom");
        self.written.retain(|key, _| {
            enabled.iter().any(|(enabled, _)| key == enabled)
                || (*key == "integration_wezterm" && wezterm_on)
                || (*key == "integration_bottom" && bottom_on)
        });
        drop(settings);
        let fingerprint = fingerprint(&theme);
        for (key, export) in enabled {
            let result = self.export_once(key, fingerprint.clone(), || {
                export(&theme);
                Ok(())
            });
            self.report(key, result);
        }
        if wezterm_on {
            let result = self.export_once("integration_wezterm", fingerprint.clone(), || {
                wezterm::export(&theme)
            });
            self.report("integration_wezterm", result);
        }
        if bottom_on {
            let result = self.export_bottom(fingerprint, &bottom::source_path(), || {
                bottom::export(&theme)
            });
            self.report("integration_bottom", result);
        }

        false
    }
}

impl Export {
    fn export_once(
        &mut self,
        key: &'static str,
        fingerprint: String,
        export: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        if self.written.get(key) == Some(&fingerprint) {
            return Ok(());
        }
        export()?;
        self.written.insert(key, fingerprint);
        Ok(())
    }
    fn export_bottom(
        &mut self,
        theme_fingerprint: String,
        source: &Path,
        export: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        let base = bottom::read_source(source)?;
        self.export_once(
            "integration_bottom",
            format!("{theme_fingerprint}\0{base}"),
            export,
        )
    }

    fn report(&mut self, key: &'static str, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.errors.remove(key);
            }
            Err(error) => {
                if self.errors.get(key) != Some(&error) {
                    eprintln!("kajitsu: {key}: {error}");
                }
                self.errors.insert(key, error);
            }
        }
    }
}

pub fn write_owned(path: &Path, text: &str) -> Result<(), String> {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let write = || -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut name = path
            .file_name()
            .ok_or(std::io::ErrorKind::InvalidInput)?
            .to_os_string();
        name.push(format!(
            ".{}.{}.partial",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let partial = path.with_file_name(name);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&partial)?;
        let result = (|| {
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&partial, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&partial);
        }
        result
    };
    write().map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn fingerprint(theme: &Theme) -> String {
    let colors = [
        theme.background,
        theme.surface,
        theme.hover_surface,
        theme.selected_surface,
        theme.border,
        theme.text,
        theme.secondary_text,
        theme.muted_text,
        theme.accent,
        theme.accent_hover,
        theme.on_accent,
        theme.success,
        theme.danger,
    ];

    let mut text = format!("{}", theme.light);

    for color in colors {
        text.push_str(&hex(color));
    }

    text
}

// "#rrggbb", never with alpha
pub fn hex(color: Color) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        color.red(),
        color.green(),
        color.blue()
    )
}

pub fn home() -> String {
    env::var("HOME").expect("failed to find home: HOME is not set")
}

pub fn config_home() -> String {
    env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|path| Path::new(path).is_absolute())
        .unwrap_or_else(|| format!("{}/.config", home()))
}

// where the generated files go, next to the settings
pub fn state_home() -> String {
    crate::paths::state_dir().to_string_lossy().into_owned()
}

/*
 * written next to the file and renamed over it, so a program reading it
 * never sees half a file; a failed write only leaves the old colors
 */
pub fn write(path: &str, text: &str) {
    if let Err(error) = write_owned(Path::new(path), text) {
        eprintln!("kajitsu: {error}");
    }
}

// every @NAME@ in the template swapped for its color
pub fn fill(template: &str, names: &[(&str, String)]) -> String {
    let mut text = String::from(template);

    for (name, value) in names {
        text = text.replace(name, value);
    }

    text
}

#[cfg(test)]
pub(super) fn test_theme(light: bool) -> Theme {
    Theme {
        light,
        background: Color::rgb(1, 2, 3),
        surface: Color::from("#112233"),
        hover_surface: Color::from("#334455"),
        selected_surface: Color::from("#223344"),
        border: Color::from("#445566"),
        text: Color::from("#f1f2f3"),
        secondary_text: Color::from("#aabbcc"),
        muted_text: Color::from("#778899"),
        accent: Color::from("#89b4fa"),
        accent_hover: Color::from("#99c4ff"),
        on_accent: Color::from("#111111"),
        success: Color::from("#a6e3a1"),
        danger: Color::from("#f38ba8"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn failed_atomic_export_keeps_previous_file_and_retries() {
        let folder = std::env::temp_dir().join(format!("kajitsu-write-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("palette.lua");
        fs::write(&path, "old valid palette").unwrap();
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
        let mut exporter = Export::default();
        let result = exporter.export_once("wezterm", "new".into(), || {
            write_owned(&path, "new palette")
        });
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "old valid palette");
        assert!(!exporter.written.contains_key("wezterm"));
        exporter
            .export_once("wezterm", "new".into(), || {
                write_owned(&path, "new palette")
            })
            .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new palette");
        fs::remove_dir_all(folder).unwrap();
    }
}
