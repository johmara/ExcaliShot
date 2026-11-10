//! Persisted user settings (currently the shortcut choices on platforms where the app owns
//! the shortcuts: macOS and Windows). Plain JSON in the app config directory.
#![cfg_attr(target_os = "linux", allow(dead_code))]

use std::{collections::BTreeMap, path::Path};

use serde::{Deserialize, Serialize};

use crate::shortcuts::{DEFS, normalize_accelerator};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Settings {
    /// Shortcut id ("region", "monitor", "full") to accelerator ("Ctrl+Shift+1").
    #[serde(default)]
    pub shortcuts: BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            shortcuts: DEFS
                .iter()
                .map(|(id, _, trigger, _)| (id.to_string(), normalize_accelerator(trigger)))
                .collect(),
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
        s.save(&path).unwrap();
        let back = Settings::load(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!(back, s);
    }
}
