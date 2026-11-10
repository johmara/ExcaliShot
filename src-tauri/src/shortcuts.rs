//! Global shortcuts.
//!
//! Linux: the xdg-desktop-portal GlobalShortcuts interface (works on GNOME Wayland, KDE).
//! The desktop owns the shortcut assignment UI; we propose defaults.
//! macOS and Windows: `tauri-plugin-global-shortcut`, with the choices stored in
//! `settings.json` and edited in our own settings window. (Compiled by CI, untested on
//! real hardware so far.)

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::{flow::Mode, settings::Settings};

pub const APP_ID: &str = "se.johmara.excalishot";

/// id, description, preferred trigger, capture mode.
pub const DEFS: [(&str, &str, &str, Mode); 3] = [
    ("region", "Capture region", "CTRL+SHIFT+1", Mode::Region),
    ("monitor", "Capture current monitor", "CTRL+SHIFT+2", Mode::Monitor),
    ("full", "Capture all monitors", "CTRL+SHIFT+3", Mode::Full),
];

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Serialize, Clone)]
pub struct ShortcutDto {
    id: String,
    description: String,
    trigger: String,
    /// Set when the shortcut could not be registered (invalid, or already taken by another app).
    problem: Option<String>,
}

#[derive(Serialize, Clone, Default)]
pub struct ShortcutStatus {
    backend: String,
    /// Whether the settings window may change the shortcuts (false when the desktop owns them).
    editable: bool,
    error: Option<String>,
    shortcuts: Vec<ShortcutDto>,
}

impl ShortcutStatus {
    /// Human-readable trigger (e.g. "Ctrl+Shift+1") for a shortcut id, if one is assigned.
    pub fn trigger_for(&self, id: &str) -> Option<String> {
        self.shortcuts
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.trigger.clone())
            .filter(|t| !t.is_empty())
    }
}

/// Canonical spelling of an accelerator: "ctrl+shift+a" -> "Ctrl+Shift+A".
pub fn normalize_accelerator(raw: &str) -> String {
    raw.split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| match p.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "Ctrl".to_string(),
            "alt" | "option" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            "super" | "win" | "windows" | "meta" => "Super".to_string(),
            "cmd" | "command" => "Cmd".to_string(),
            "commandorcontrol" | "cmdorctrl" => "CommandOrControl".to_string(),
            _ if p.chars().count() == 1 => p.to_uppercase(),
            _ => p.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Normalise portal trigger descriptions ("Press <Shift><Control>1") to "Ctrl+Shift+1".
/// Anything without `<Mod>` tokens is returned trimmed and unchanged.
pub fn pretty_trigger(raw: &str) -> String {
    let s = raw.trim();
    let s = s.strip_prefix("Press ").unwrap_or(s).trim();
    if !s.contains('<') {
        return s.to_string();
    }
    let key = s.rsplit('>').next().unwrap_or("");
    let has = |names: &[&str]| names.iter().any(|n| s.contains(n));
    let mut parts: Vec<&str> = Vec::new();
    if has(&["<Control>", "<Ctrl>", "<Primary>"]) {
        parts.push("Ctrl");
    }
    if has(&["<Alt>", "<Mod1>"]) {
        parts.push("Alt");
    }
    if has(&["<Shift>"]) {
        parts.push("Shift");
    }
    if has(&["<Super>", "<Meta>"]) {
        parts.push("Super");
    }
    parts.push(key);
    parts.join("+")
}

#[cfg(test)]
mod tests {
    use super::{normalize_accelerator, pretty_trigger};

    #[test]
    fn normalises_gnome_triggers() {
        assert_eq!(pretty_trigger("Press <Shift><Control>1"), "Ctrl+Shift+1");
        assert_eq!(pretty_trigger("<Alt><Super>x"), "Alt+Super+x");
    }

    #[test]
    fn leaves_plain_triggers_alone() {
        assert_eq!(pretty_trigger("Ctrl+Shift+1"), "Ctrl+Shift+1");
        assert_eq!(pretty_trigger(""), "");
    }

    #[test]
    fn normalises_accelerators() {
        assert_eq!(normalize_accelerator("CTRL+SHIFT+1"), "Ctrl+Shift+1");
        assert_eq!(normalize_accelerator("ctrl + shift + a"), "Ctrl+Shift+A");
        assert_eq!(normalize_accelerator("Control+Option+ArrowUp"), "Ctrl+Alt+ArrowUp");
        assert_eq!(normalize_accelerator("command+Shift+4"), "Cmd+Shift+4");
        assert_eq!(normalize_accelerator(""), "");
    }
}

// ---- Linux: xdg-desktop-portal --------------------------------------------

#[cfg(target_os = "linux")]
mod portal {
    use super::*;
    use std::sync::Arc;

    use ashpd::desktop::{
        Session,
        global_shortcuts::{GlobalShortcuts, NewShortcut},
    };
    use futures_util::StreamExt;

    pub struct Handle {
        gs: GlobalShortcuts,
        session: Session<GlobalShortcuts>,
    }

    #[derive(Default)]
    pub struct PortalState {
        pub handle: std::sync::Mutex<Option<Arc<Handle>>>,
    }

    pub async fn init(app: AppHandle) -> Result<Vec<ShortcutDto>, String> {
        // GlobalShortcuts needs an app id; unsandboxed apps register one per D-Bus connection.
        // Use a dedicated connection so the Screenshot portal (which uses ashpd's shared
        // connection) keeps its original, already-permitted identity.
        let conn = ashpd::zbus::Connection::session().await.map_err(err)?;
        let id: ashpd::AppID = APP_ID.parse().map_err(err)?;
        if let Err(e) = ashpd::register_host_app_with_connection(conn.clone(), id).await {
            eprintln!("portal app registration failed (continuing): {e}");
        }

        let gs = GlobalShortcuts::with_connection(conn).await.map_err(err)?;
        let session = gs.create_session(Default::default()).await.map_err(err)?;
        let new: Vec<NewShortcut> = DEFS
            .iter()
            .map(|(id, desc, trig, _)| NewShortcut::new(*id, *desc).preferred_trigger(*trig))
            .collect();
        let bound = gs
            .bind_shortcuts(&session, &new, None, Default::default())
            .await
            .map_err(err)?
            .response()
            .map_err(err)?;
        let dtos = bound
            .shortcuts()
            .iter()
            .map(|s| ShortcutDto {
                id: s.id().to_owned(),
                description: s.description().to_owned(),
                trigger: pretty_trigger(s.trigger_description()),
                problem: None,
            })
            .collect();

        let mut activated = Box::pin(gs.receive_activated().await.map_err(err)?);
        let listener_app = app.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(a) = activated.next().await {
                if let Some((_, _, _, mode)) = DEFS.iter().find(|d| d.0 == a.shortcut_id()) {
                    crate::trigger(&listener_app, *mode);
                }
            }
        });

        let state = app.state::<PortalState>();
        *state.handle.lock().unwrap() = Some(Arc::new(Handle { gs, session }));
        Ok(dtos)
    }

    pub async fn configure(state: State<'_, PortalState>) -> Result<(), String> {
        let h = state.handle.lock().unwrap().clone().ok_or("shortcut portal is not initialised")?;
        h.gs.configure_shortcuts(&h.session, None, Default::default())
            .await
            .map_err(err)
    }
}

#[cfg(target_os = "linux")]
pub use portal::PortalState;
#[cfg(not(target_os = "linux"))]
#[derive(Default)]
pub struct PortalState;

// ---- macOS / Windows: tauri-plugin-global-shortcut -------------------------

#[cfg(not(target_os = "linux"))]
mod native {
    use super::*;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    pub fn config_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
        Ok(app.path().app_config_dir().map_err(err)?.join("settings.json"))
    }

    pub fn validate(accelerator: &str) -> Result<(), String> {
        accelerator.parse::<Shortcut>().map(|_| ()).map_err(err)
    }

    /// (Re)register every configured shortcut. A shortcut that cannot be registered (invalid, or
    /// already taken by another app) is reported on its entry instead of failing the others.
    pub fn register_all(app: &AppHandle, cfg: &Settings) -> Vec<ShortcutDto> {
        let gs = app.global_shortcut();
        let _ = gs.unregister_all();
        DEFS.iter()
            .map(|(id, description, _, mode)| {
                let trigger = cfg.shortcuts.get(*id).cloned().unwrap_or_default();
                let mode = *mode;
                let problem = match trigger.parse::<Shortcut>() {
                    Err(e) => Some(format!("invalid shortcut: {e}")),
                    Ok(shortcut) => gs
                        .on_shortcut(shortcut, move |app, _shortcut, event| {
                            if event.state() == ShortcutState::Pressed {
                                crate::trigger(app, mode);
                            }
                        })
                        .err()
                        .map(|e| e.to_string()),
                };
                ShortcutDto { id: id.to_string(), description: description.to_string(), trigger, problem }
            })
            .collect()
    }
}

#[derive(Default)]
pub struct StatusState(pub std::sync::Mutex<ShortcutStatus>);

/// Only read on macOS/Windows, where the app stores the shortcut choices itself.
#[derive(Default)]
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub struct SettingsState(pub std::sync::Mutex<Settings>);

/// Called once at startup.
pub async fn init(app: AppHandle) {
    let status = {
        #[cfg(target_os = "linux")]
        {
            match portal::init(app.clone()).await {
                Ok(shortcuts) => ShortcutStatus {
                    backend: "xdg-desktop-portal".into(),
                    editable: false,
                    error: None,
                    shortcuts,
                },
                Err(e) => ShortcutStatus {
                    backend: "xdg-desktop-portal".into(),
                    editable: false,
                    error: Some(e),
                    shortcuts: vec![],
                },
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let cfg = native::config_path(&app).map(|p| Settings::load(&p)).unwrap_or_default();
            let shortcuts = native::register_all(&app, &cfg);
            *app.state::<SettingsState>().0.lock().unwrap() = cfg;
            let problems: Vec<String> = shortcuts
                .iter()
                .filter_map(|s| s.problem.as_ref().map(|p| format!("{}: {p}", s.description)))
                .collect();
            ShortcutStatus {
                backend: "global-shortcut".into(),
                editable: true,
                error: (!problems.is_empty()).then(|| problems.join("; ")),
                shortcuts,
            }
        }
    };
    if let Some(e) = &status.error {
        eprintln!("global shortcuts unavailable: {e}");
    }
    *app.state::<StatusState>().0.lock().unwrap() = status;
    crate::refresh_tray(&app);
}

#[tauri::command]
pub fn shortcut_status(state: State<'_, StatusState>) -> ShortcutStatus {
    state.0.lock().unwrap().clone()
}

/// Open the desktop's own shortcut configuration (GNOME/KDE) for this app. Linux only.
#[tauri::command]
pub async fn configure_shortcuts(#[allow(unused)] state: State<'_, PortalState>) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        portal::configure(state).await
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("not supported on this platform".into())
    }
}

/// Change one shortcut (macOS and Windows). `accelerator` looks like "Ctrl+Shift+1".
#[tauri::command]
pub fn set_shortcut(
    app: AppHandle,
    status: State<'_, StatusState>,
    settings: State<'_, SettingsState>,
    id: String,
    accelerator: String,
) -> Result<ShortcutStatus, String> {
    #[cfg(target_os = "linux")]
    {
        let _ = (&app, &status, &settings, &id, &accelerator);
        Err("On Linux the desktop manages the shortcuts.".into())
    }
    #[cfg(not(target_os = "linux"))]
    {
        if !DEFS.iter().any(|d| d.0 == id) {
            return Err("unknown shortcut".into());
        }
        let accelerator = normalize_accelerator(&accelerator);
        native::validate(&accelerator)?;

        let shortcuts = {
            let mut cfg = settings.0.lock().unwrap();
            if cfg.shortcuts.iter().any(|(k, v)| *k != id && v.eq_ignore_ascii_case(&accelerator)) {
                return Err("That shortcut is already used by another ExcaliShot action.".into());
            }
            cfg.shortcuts.insert(id, accelerator);
            cfg.save(&native::config_path(&app)?)?;
            native::register_all(&app, &cfg)
        };
        let problems: Vec<String> = shortcuts
            .iter()
            .filter_map(|s| s.problem.as_ref().map(|p| format!("{}: {p}", s.description)))
            .collect();
        let new_status = ShortcutStatus {
            backend: "global-shortcut".into(),
            editable: true,
            error: (!problems.is_empty()).then(|| problems.join("; ")),
            shortcuts,
        };
        *status.0.lock().unwrap() = new_status.clone();
        crate::refresh_tray(&app);
        Ok(new_status)
    }
}
