//! Global shortcuts.
//!
//! Linux: the xdg-desktop-portal GlobalShortcuts interface (works on GNOME Wayland, KDE).
//! GNOME owns the shortcut assignment UI; we propose defaults and can open its settings.
//! macOS/Windows/X11-only: TODO(step 6b) tauri-plugin-global-shortcut backend + our own settings UI.

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::flow::Mode;

pub const APP_ID: &str = "se.johmara.excalishot";

/// id, description, XDG-style preferred trigger, capture mode.
pub const DEFS: [(&str, &str, &str, Mode); 3] = [
    ("region", "Capture region", "CTRL+SHIFT+1", Mode::Region),
    ("monitor", "Capture current monitor", "CTRL+SHIFT+2", Mode::Monitor),
    ("full", "Capture all monitors", "CTRL+SHIFT+3", Mode::Full),
];

#[derive(Serialize, Clone)]
pub struct ShortcutDto {
    id: String,
    description: String,
    trigger: String,
}

#[derive(Serialize, Clone, Default)]
pub struct ShortcutStatus {
    backend: String,
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
    use super::pretty_trigger;

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
}

#[cfg(target_os = "linux")]
mod portal {
    use super::*;
    use std::sync::Arc;

    use ashpd::desktop::{
        Session,
        global_shortcuts::{GlobalShortcuts, NewShortcut},
    };
    use futures_util::StreamExt;

    fn err<E: std::fmt::Display>(e: E) -> String {
        e.to_string()
    }

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

        let state = app_state(&app);
        *state.handle.lock().unwrap() = Some(Arc::new(Handle { gs, session }));
        Ok(dtos)
    }

    fn app_state(app: &AppHandle) -> tauri::State<'_, PortalState> {
        use tauri::Manager as _;
        app.state::<PortalState>()
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

#[derive(Default)]
pub struct StatusState(pub std::sync::Mutex<ShortcutStatus>);

/// Called once at startup.
pub async fn init(app: AppHandle) {
    use tauri::Manager as _;
    let status = {
        #[cfg(target_os = "linux")]
        {
            match portal::init(app.clone()).await {
                Ok(shortcuts) => ShortcutStatus { backend: "xdg-desktop-portal".into(), error: None, shortcuts },
                Err(e) => ShortcutStatus { backend: "xdg-desktop-portal".into(), error: Some(e), shortcuts: vec![] },
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            ShortcutStatus {
                backend: "none".into(),
                error: Some("global shortcuts are not implemented on this platform yet".into()),
                shortcuts: vec![],
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

/// Open the desktop's own shortcut configuration (GNOME/KDE) for this app.
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
