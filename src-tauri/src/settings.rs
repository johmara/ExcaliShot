//! Persisted user settings: the shortcut choices (only used on platforms where the app owns the
//! shortcuts, macOS and Windows) and the annotate-on-screen option. Plain JSON in the app config
//! directory.

use std::{collections::BTreeMap, path::Path};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::shortcuts::{DEFS, normalize_accelerator};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Settings {
    /// Shortcut id ("region", "monitor", "full") to accelerator ("Ctrl+Shift+1").
    #[serde(default)]
    pub shortcuts: BTreeMap<String, String>,
    /// Annotate directly on the frozen screen after a region selection (Flameshot style).
    #[serde(default)]
    pub annotate_on_screen: bool,
}

/// Where `settings.json` lives.
pub fn config_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("settings.json"))
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            shortcuts: DEFS
                .iter()
                .map(|(id, _, trigger, _)| (id.to_string(), normalize_accelerator(trigger)))
                .collect(),
            annotate_on_screen: false,
        }
    }
}

impl Settings {
    /// Read settings; a missing or unreadable file gives the defaults, and any shortcut the
    /// file does not mention gets its default.
    pub fn load(path: &Path) -> Settings {
        let mut s: Settings = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        for (id, accel) in Settings::default().shortcuts {
            s.shortcuts.entry(id).or_insert(accel);
        }
        s
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("excalishot-{}-{name}.json", std::process::id()))
    }

    #[test]
    fn missing_file_gives_defaults() {
        let s = Settings::load(&temp_file("does-not-exist"));
        assert_eq!(s, Settings::default());
        assert_eq!(s.shortcuts["region"], "Ctrl+Shift+1");
    }

    #[test]
    fn partial_file_is_filled_with_defaults() {
        let path = temp_file("partial");
        std::fs::write(&path, r#"{"shortcuts":{"region":"Alt+R"}}"#).unwrap();
        let s = Settings::load(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!(s.shortcuts["region"], "Alt+R");
        assert_eq!(s.shortcuts["monitor"], "Ctrl+Shift+2");
        assert_eq!(s.shortcuts["full"], "Ctrl+Shift+3");
    }

    #[test]
    fn annotate_defaults_to_off_and_old_files_still_load() {
        let path = temp_file("old-format");
        std::fs::write(&path, r#"{"shortcuts":{"region":"Alt+R"}}"#).unwrap();
        let s = Settings::load(&path);
        std::fs::remove_file(&path).ok();
        assert!(!s.annotate_on_screen);
    }

    #[test]
    fn corrupt_file_gives_defaults() {
        let path = temp_file("corrupt");
        std::fs::write(&path, "not json").unwrap();
        let s = Settings::load(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn round_trips() {
        let path = temp_file("roundtrip");
        let mut s = Settings::default();
        s.shortcuts.insert("full".into(), "Ctrl+Alt+F".into());
        s.annotate_on_screen = true;
        s.save(&path).unwrap();
        let back = Settings::load(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!(back, s);
    }
}
