use std::path::PathBuf;

pub fn config_dir() -> PathBuf { resolve("config", &|key| std::env::var_os(key)) }
pub fn state_dir() -> PathBuf { resolve("state", &|key| std::env::var_os(key)) }
pub fn cache_dir() -> PathBuf { resolve("cache", &|key| std::env::var_os(key)) }

fn resolve(kind: &str, env: &impl Fn(&str) -> Option<std::ffi::OsString>) -> PathBuf {
    let absolute = |key| env(key).map(PathBuf::from).filter(|path| path.is_absolute());
    if kind == "config" && let Some(path) = absolute("KAJITSU_CONFIG_DIR") {
        return path;
    }
    let (key, fallback) = match kind {
        "config" => ("XDG_CONFIG_HOME", ".config"),
        "state" => ("XDG_STATE_HOME", ".local/state"),
        _ => ("XDG_CACHE_HOME", ".cache"),
    };
    absolute(key).unwrap_or_else(|| {
        PathBuf::from(env("HOME").expect("HOME must be set")).join(fallback)
    }).join("kajitsu")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path(kind: &str, vars: &[(&str, &str)]) -> PathBuf {
        resolve(kind, &|key| vars.iter().find(|(name, _)| *name == key).map(|(_, value)| (*value).into()))
    }
    #[test]
    fn separates_config_state_and_cache_with_xdg_overrides() {
        let vars = [("HOME", "/home/test"), ("XDG_CONFIG_HOME", "/config"), ("XDG_STATE_HOME", "/state"), ("XDG_CACHE_HOME", "/cache"), ("KAJITSU_CONFIG_DIR", "/checkout")];
        assert_eq!(path("config", &vars), PathBuf::from("/checkout"));
        assert_eq!(path("state", &vars), PathBuf::from("/state/kajitsu"));
        assert_eq!(path("cache", &vars), PathBuf::from("/cache/kajitsu"));
    }
    #[test]
    fn empty_or_relative_xdg_values_fall_back_to_home() {
        let vars = [("HOME", "/home/test"), ("XDG_CONFIG_HOME", ""), ("XDG_STATE_HOME", "relative")];
        assert_eq!(path("config", &vars), PathBuf::from("/home/test/.config/kajitsu"));
        assert_eq!(path("state", &vars), PathBuf::from("/home/test/.local/state/kajitsu"));
        assert_eq!(path("cache", &vars), PathBuf::from("/home/test/.cache/kajitsu"));
    }
}
