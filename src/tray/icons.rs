use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use amane::Image;

use super::backend::lock;
use super::model::{IconSources, IconSpec, ItemIcon, RawPixmap};

pub(crate) fn argb_to_rgba(width: i32, height: i32, bytes: &[u8]) -> Option<Vec<u8>> {
    if !(1..=1024).contains(&width) || !(1..=1024).contains(&height) {
        return None;
    }
    let length = usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(4)?;
    if length != bytes.len() {
        return None;
    }
    Some(
        bytes
            .chunks(4)
            .flat_map(|p| [p[1], p[2], p[3], p[0]])
            .collect(),
    )
}

fn choose_pixmap(pixmaps: &[RawPixmap], target: u32) -> Option<Image> {
    let mut candidates: Vec<_> = pixmaps
        .iter()
        .filter(|(w, h, bytes)| {
            (1..=1024).contains(w)
                && (1..=1024).contains(h)
                && bytes.len() == *w as usize * *h as usize * 4
        })
        .collect();
    candidates.sort_by_key(|(w, h, _)| {
        let edge = (*w).min(*h) as u32;
        (edge < target, edge.abs_diff(target))
    });
    for (width, height, bytes) in candidates {
        if let Some(pixels) = argb_to_rgba(*width, *height, bytes) {
            return Image::from_rgba(*width as u32, *height as u32, pixels);
        }
    }
    None
}

type LookupKey = (String, Option<PathBuf>, u32);

struct IconLookup {
    theme: String,
    roots: Vec<PathBuf>,
    pixmaps: Vec<PathBuf>,
    cache: HashMap<LookupKey, Vec<PathBuf>>,
    refreshed: Instant,
}

impl IconLookup {
    fn new(theme: String, roots: Vec<PathBuf>, pixmaps: Vec<PathBuf>) -> Self {
        Self {
            theme,
            roots,
            pixmaps,
            cache: HashMap::new(),
            refreshed: Instant::now(),
        }
    }

    fn clear(&mut self) {
        self.cache.clear();
    }

    fn desktop() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"));
        let mut roots = vec![data.join("icons"), home.join(".icons")];
        let directories = std::env::var_os("XDG_DATA_DIRS")
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        let mut pixmaps = Vec::new();
        for root in std::env::split_paths(&directories).filter(|p| p.is_absolute()) {
            roots.push(root.join("icons"));
            pixmaps.push(root.join("pixmaps"));
        }
        Self::new(desktop_theme(), roots, pixmaps)
    }

    fn candidates(&mut self, name: &str, app: Option<&Path>, target: u32) -> Vec<PathBuf> {
        if Path::new(name).is_absolute() {
            return vec![PathBuf::from(name)];
        }
        if name.is_empty() || name.contains('/') {
            return Vec::new();
        }
        let key = (name.into(), app.map(Path::to_path_buf), target);
        if let Some(paths) = self.cache.get(&key) {
            return paths.clone();
        }
        let mut paths = Vec::new();
        if let Some(app) = app {
            append_names(&mut paths, app, name);
            let (_, directories) = theme_index(app, target);
            for (_, dir) in directories {
                append_names(&mut paths, &app.join(dir), name);
            }
        }
        let mut queue = VecDeque::from([self.theme.clone()]);
        let mut visited = HashSet::new();
        while let Some(theme) = queue.pop_front() {
            if !visited.insert(theme.clone()) {
                if queue.is_empty() && !visited.contains("hicolor") {
                    queue.push_back("hicolor".into());
                }
                continue;
            }
            let mut candidates = Vec::new();
            for root in &self.roots {
                let root = root.join(&theme);
                let (inherits, directories) = theme_index(&root, target);
                queue.extend(inherits);
                for (distance, dir) in directories {
                    candidates.push((distance, root.join(dir)));
                }
            }
            candidates.sort_by_key(|(distance, _)| *distance);
            for (_, dir) in candidates {
                append_names(&mut paths, &dir, name);
            }
            if queue.is_empty() && !visited.contains("hicolor") {
                queue.push_back("hicolor".into());
            }
        }
        for root in &self.pixmaps {
            append_names(&mut paths, root, name);
        }
        if self.cache.len() >= 256 {
            self.clear();
        }
        self.cache.insert(key, paths.clone());
        paths
    }
}

fn desktop_theme() -> String {
    std::env::var("GTK_ICON_THEME")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            let output = Command::new("gsettings")
                .args(["get", "org.gnome.desktop.interface", "icon-theme"])
                .output()
                .ok()?;
            output.status.success().then(|| {
                String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .trim_matches('\'')
                    .to_string()
            })
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "hicolor".into())
}

fn append_names(paths: &mut Vec<PathBuf>, directory: &Path, name: &str) {
    paths.push(directory.join(format!("{name}.png")));
    paths.push(directory.join(format!("{name}.svg")));
    if name.ends_with(".png") || name.ends_with(".svg") {
        paths.push(directory.join(name));
    }
}

fn theme_index(root: &Path, target: u32) -> (Vec<String>, Vec<(u64, PathBuf)>) {
    let Ok(text) = fs::read_to_string(root.join("index.theme")) else {
        return (Vec::new(), Vec::new());
    };
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut section = String::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
    {
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = name.into();
        } else if let Some((key, value)) = line.split_once('=') {
            sections
                .entry(section.clone())
                .or_default()
                .insert(key.trim().into(), value.trim().into());
        }
    }
    let Some(theme) = sections.get("Icon Theme") else {
        return (Vec::new(), Vec::new());
    };
    let inherits = theme
        .get("Inherits")
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let mut directories = Vec::new();
    for name in ["Directories", "ScaledDirectories"]
        .into_iter()
        .filter_map(|key| theme.get(key))
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let path = PathBuf::from(name);
        if path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            continue;
        }
        let Some(values) = sections.get(name) else {
            continue;
        };
        let number = |key, default| {
            values
                .get(key)
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(default)
        };
        let size = number("Size", 18);
        let scale = number("Scale", 1).max(1);
        let (min, max) = match values
            .get("Type")
            .map(String::as_str)
            .unwrap_or("Threshold")
        {
            "Scalable" => (number("MinSize", size), number("MaxSize", size)),
            "Fixed" => (size, size),
            _ => (
                size.saturating_sub(number("Threshold", 2)),
                size.saturating_add(number("Threshold", 2)),
            ),
        };
        let min = min.saturating_mul(scale);
        let max = max.saturating_mul(scale).max(min);
        let target = u64::from(target);
        let distance = if target < min {
            min - target
        } else {
            target.saturating_sub(max)
        };
        directories.push((distance, path));
    }
    directories.sort_by_key(|(distance, _)| *distance);
    (inherits, directories)
}

static LOOKUP: LazyLock<Mutex<IconLookup>> = LazyLock::new(|| Mutex::new(IconLookup::desktop()));

fn resolve_spec(
    lookup: &mut IconLookup,
    spec: &IconSpec,
    app: Option<&Path>,
    target: u32,
) -> Option<Image> {
    for path in lookup.candidates(&spec.name, app, target) {
        if let Some(image) = Image::read_owned(&path) {
            return Some(image);
        }
    }
    choose_pixmap(&spec.pixmaps, target)
}

pub(crate) fn resolve(sources: &IconSources, target_px: u32) -> ItemIcon {
    let mut lookup = lock(&LOOKUP);
    if lookup.refreshed.elapsed() >= Duration::from_secs(5) {
        let theme = desktop_theme();
        if lookup.theme != theme {
            lookup.theme = theme;
            lookup.clear();
        }
        lookup.refreshed = Instant::now();
    }
    let app = sources.theme_path.as_deref();
    ItemIcon {
        normal: resolve_spec(&mut lookup, &sources.normal, app, target_px),
        attention: resolve_spec(&mut lookup, &sources.attention, app, target_px),
        overlay: resolve_spec(&mut lookup, &sources.overlay, app, target_px),
    }
}

static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(crate) fn read_menu_icon(bytes: &[u8]) -> Option<Image> {
    read_menu_icon_in(bytes, &std::env::temp_dir())
}

fn read_menu_icon_in(bytes: &[u8], root: &Path) -> Option<Image> {
    if bytes.len() > 4 * 1024 * 1024 || !bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]) {
        return None;
    }
    let path = root.join(format!(
        "kajitsu-menu-{}-{}.png",
        std::process::id(),
        NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .ok()?;
    let scratch = Scratch(path);
    file.write_all(bytes).ok()?;
    drop(file);
    Image::read_owned(&scratch.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn pixmaps_validate_before_conversion() {
        assert_eq!(
            argb_to_rgba(1, 1, &[0x80, 0x11, 0x22, 0x33]),
            Some(vec![0x11, 0x22, 0x33, 0x80])
        );
        for (width, height, bytes) in [
            (1025, 1, vec![]),
            (-1, 1, vec![]),
            (0, 1, vec![]),
            (1, 1, vec![0; 3]),
        ] {
            assert!(argb_to_rgba(width, height, &bytes).is_none());
        }
        let spec = IconSpec {
            name: String::new(),
            pixmaps: vec![
                (18, 18, vec![255; 18 * 18 * 4]),
                (32, 32, vec![128; 32 * 32 * 4]),
            ],
        };
        assert!(
            choose_pixmap(&spec.pixmaps, 18).unwrap()
                == Image::from_rgba(18, 18, vec![255; 18 * 18 * 4]).unwrap()
        );
        assert!(
            choose_pixmap(&spec.pixmaps, 23).unwrap()
                == Image::from_rgba(32, 32, vec![128; 32 * 32 * 4]).unwrap()
        );
    }

    #[test]
    fn theme_cycles_and_bad_named_icons_fall_back() {
        let folder = std::env::temp_dir().join(format!("kajitsu-theme-{}", std::process::id()));
        for theme in ["A", "B"] {
            fs::create_dir_all(folder.join(theme).join("18/apps")).unwrap();
            fs::write(folder.join(theme).join("index.theme"), format!("[Icon Theme]\nInherits={}\nDirectories=18/apps\n[18/apps]\nSize=18\nType=Fixed\n", if theme == "A" { "B" } else { "A" })).unwrap();
        }
        fs::write(folder.join("A/18/apps/mail.png"), "broken PNG").unwrap();
        let mut lookup = IconLookup::new("A".into(), vec![folder.clone()], vec![]);
        let spec = IconSpec {
            name: "mail".into(),
            pixmaps: vec![(1, 1, vec![255, 17, 34, 51])],
        };
        assert!(resolve_spec(&mut lookup, &spec, None, 18).is_some());
        fs::write(folder.join("B/18/apps/mail.svg"), r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#).unwrap();
        lookup.clear();
        let icon = resolve_spec(
            &mut lookup,
            &IconSpec {
                name: "mail".into(),
                pixmaps: vec![],
            },
            None,
            18,
        )
        .unwrap();
        assert!(icon == Image::read_owned(folder.join("B/18/apps/mail.svg")).unwrap());
        fs::create_dir_all(folder.join("hicolor/18/apps")).unwrap();
        fs::write(
            folder.join("hicolor/index.theme"),
            "[Icon Theme]\nDirectories=18/apps\n[18/apps]\nSize=18\nType=Fixed\n",
        )
        .unwrap();
        fs::write(folder.join("hicolor/18/apps/hicolor-only.svg"), r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="blue"/></svg>"#).unwrap();
        lookup.clear();
        assert!(
            resolve_spec(
                &mut lookup,
                &IconSpec {
                    name: "hicolor-only".into(),
                    pixmaps: vec![]
                },
                None,
                18
            )
            .is_some()
        );
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn menu_icons_are_owned_and_scratch_files_are_removed() {
        let folder =
            std::env::temp_dir().join(format!("kajitsu-menu-image-{}", std::process::id()));
        fs::create_dir(&folder).unwrap();
        let png = [
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207,
            192, 240, 31, 0, 5, 0, 1, 255, 114, 156, 82, 103, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
            96, 130,
        ];
        assert!(read_menu_icon_in(&png, &folder).is_some());
        assert!(read_menu_icon_in(b"broken", &folder).is_none());
        assert!(read_menu_icon_in(&vec![0; 4 * 1024 * 1024 + 1], &folder).is_none());
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 0);
        fs::remove_dir(folder).unwrap();
    }
}
