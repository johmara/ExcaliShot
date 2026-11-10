//! In-app updates: signed releases on GitHub (`latest.json`) through `tauri-plugin-updater`.
//!
//! Only real releases are offered: the endpoint is `releases/latest`, which GitHub resolves to
//! the newest non-pre-release, so `-rc` builds from `main` never show up here.
//!
//! On Linux the updater can only replace an AppImage. A `.deb`/`.rpm` install is told to
//! download the new version instead (`can_install` is false).

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate,
    #[serde(rename_all = "camelCase")]
    Available { version: String, notes: Option<String>, can_install: bool },
    Installing,
    #[serde(rename_all = "camelCase")]
    Error { message: String },
}

#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<Update>>,
    status: Mutex<UpdateStatus>,
}

/// Whether this installation can replace itself. Linux packages (.deb/.rpm) cannot; only an
/// AppImage can (`APPIMAGE` is set when running from one).
fn can_install() -> bool {
    !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some()
}

fn tray_text(status: &UpdateStatus) -> String {
    match status {
        UpdateStatus::Available { version, .. } => format!("Update available: v{version}..."),
        UpdateStatus::Checking => "Checking for updates...".to_string(),
        UpdateStatus::Installing => "Installing update...".to_string(),
        _ => "Check for updates...".to_string(),
    }
}

/// Store the new status, tell the settings window, and relabel the tray entry.
fn set_status(app: &AppHandle, status: UpdateStatus) {
    if let Some(items) = app.try_state::<crate::TrayItems>() {
        let _ = items.update.set_text(tray_text(&status));
    }
    let _ = app.emit("update-status", &status);
    *app.state::<UpdateState>().status.lock().unwrap() = status;
}

/// Ask the update server whether a newer release exists, and record the answer.
pub async fn check(app: &AppHandle) {
    set_status(app, UpdateStatus::Checking);
    let result: Result<Option<Update>, String> = async {
        let updater = app.updater().map_err(err)?;
        updater.check().await.map_err(err)
    }
    .await;

    let state = app.state::<UpdateState>();
    match result {
        Ok(Some(update)) => {
            let status = UpdateStatus::Available {
                version: update.version.clone(),
                notes: update.body.clone(),
                can_install: can_install(),
            };
            *state.pending.lock().unwrap() = Some(update);
            set_status(app, status);
        }
        Ok(None) => {
            *state.pending.lock().unwrap() = None;
            set_status(app, UpdateStatus::UpToDate);
        }
        Err(message) => {
            eprintln!("update check failed: {message}");
            set_status(app, UpdateStatus::Error { message });
        }
    }
}

#[tauri::command]
pub fn update_status(state: State<'_, UpdateState>) -> UpdateStatus {
    state.status.lock().unwrap().clone()
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<(), String> {
    check(&app).await;
    Ok(())
}

/// Download, verify (signature) and install the pending update, then restart the app.
#[tauri::command]
pub async fn install_update(app: AppHandle, state: State<'_, UpdateState>) -> Result<(), String> {
    if !can_install() {
        return Err("This installation cannot update itself. Download the new version from GitHub.".into());
    }
    let update = state.pending.lock().unwrap().take().ok_or("no update to install")?;
    set_status(&app, UpdateStatus::Installing);
    match update.download_and_install(|_chunk, _total| {}, || {}).await {
        Ok(()) => app.restart(),
        Err(e) => {
            let message = err(e);
            set_status(&app, UpdateStatus::Error { message: message.clone() });
            Err(message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serialises_with_a_state_tag() {
        let json = |s: &UpdateStatus| serde_json::to_value(s).unwrap();
        assert_eq!(json(&UpdateStatus::UpToDate), serde_json::json!({ "state": "upToDate" }));
        assert_eq!(
            json(&UpdateStatus::Available { version: "1.2.3".into(), notes: None, can_install: true }),
            serde_json::json!({ "state": "available", "version": "1.2.3", "notes": null, "canInstall": true })
        );
        assert_eq!(
            json(&UpdateStatus::Error { message: "boom".into() }),
            serde_json::json!({ "state": "error", "message": "boom" })
        );
    }

    #[test]
    fn tray_label_reflects_status() {
        assert_eq!(tray_text(&UpdateStatus::Idle), "Check for updates...");
        assert_eq!(
            tray_text(&UpdateStatus::Available { version: "0.2.0".into(), notes: None, can_install: false }),
            "Update available: v0.2.0..."
        );
    }
}
