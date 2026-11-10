//! In-app updates: signed releases on GitHub (`latest.json`) through `tauri-plugin-updater`.
//!
//! Only real releases are offered: the endpoint is `releases/latest`, which GitHub resolves to
//! the newest non-pre-release, so `-rc` builds from `main` never show up here.
//!
//! What "update" means depends on how the app was installed:
//! - macOS, Windows, Linux AppImage: the plugin replaces the app and restarts it.
//! - Linux `.deb` (Debian family): the app cannot replace itself, so it downloads the new `.deb`
//!   from the GitHub release, verifies its signature against the key in `tauri.conf.json`
//!   (CI signs the `.deb` with the same key), saves it to the Downloads folder and opens the
//!   system installer, which asks for the password.
//! - anything else: a link to the release page.

use std::{path::PathBuf, sync::Mutex};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Update, UpdaterExt};

const RELEASES_DOWNLOAD_URL: &str = "https://github.com/johmara/ExcaliShot/releases/download";
/// A package is a few tens of MB; anything far beyond that is not ours.
const MAX_PACKAGE_BYTES: usize = 200 * 1024 * 1024;

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
    Available { version: String, notes: Option<String>, can_install: bool, can_download: bool },
    Installing,
    #[serde(rename_all = "camelCase")]
    Downloading { version: String },
    #[serde(rename_all = "camelCase")]
    Downloaded { version: String, path: String },
    #[serde(rename_all = "camelCase")]
    Error { message: String },
}

#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<Update>>,
    status: Mutex<UpdateStatus>,
    /// Newest version we already told the user about with a notification.
    notified: Mutex<Option<String>>,
}

/// Whether this installation can replace itself. Linux packages (.deb/.rpm) cannot; only an
/// AppImage can (`APPIMAGE` is set when running from one).
fn can_install() -> bool {
    !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some()
}

/// The architecture word used in `.deb` file names.
fn deb_arch() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Some("amd64"),
        "aarch64" => Some("arm64"),
        _ => None,
    }
}

/// A `.deb` install on a Debian-family system can fetch and open the new package itself.
fn can_download_package() -> bool {
    cfg!(target_os = "linux")
        && std::env::var_os("APPIMAGE").is_none()
        && std::path::Path::new("/etc/debian_version").exists()
        && deb_arch().is_some()
}

/// Versions end up in a URL and a file name, so only accept plain version characters.
fn is_plain_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 40
        && v.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
}

/// Package and detached-signature URLs of a release, as named by the Tauri bundler.
fn package_urls(version: &str, arch: &str) -> (String, String) {
    let deb = format!("{RELEASES_DOWNLOAD_URL}/v{version}/ExcaliShot_{version}_{arch}.deb");
    let sig = format!("{deb}.sig");
    (deb, sig)
}

/// Check `data` against a minisign signature. Both `pubkey` and `signature` are in the form
/// Tauri uses everywhere: the minisign text, base64-encoded.
fn verify_signature(pubkey: &str, signature: &str, data: &[u8]) -> Result<(), String> {
    let decode_text = |s: &str| -> Result<String, String> {
        String::from_utf8(STANDARD.decode(s.trim()).map_err(err)?).map_err(err)
    };
    let key = minisign_verify::PublicKey::decode(&decode_text(pubkey)?).map_err(err)?;
    let sig = minisign_verify::Signature::decode(&decode_text(signature)?).map_err(err)?;
    key.verify(data, &sig, false).map_err(err)
}

/// The update public key from `plugins.updater.pubkey` in tauri.conf.json.
fn updater_pubkey(app: &AppHandle) -> Result<String, String> {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .map(str::to_string)
        .ok_or_else(|| "no updater public key in the app configuration".to_string())
}

async fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let bytes = reqwest::get(url)
        .await
        .map_err(err)?
        .error_for_status()
        .map_err(err)?
        .bytes()
        .await
        .map_err(err)?;
    if bytes.len() > MAX_PACKAGE_BYTES {
        return Err("the download is unexpectedly large".into());
    }
    Ok(bytes.to_vec())
}

fn tray_text(status: &UpdateStatus) -> String {
    match status {
        UpdateStatus::Available { version, .. } => format!("Update available: v{version}..."),
        UpdateStatus::Checking => "Checking for updates...".to_string(),
        UpdateStatus::Installing => "Installing update...".to_string(),
        UpdateStatus::Downloading { .. } => "Downloading update...".to_string(),
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

/// Tell the user once per version that an update exists. Nothing is downloaded or installed:
/// the notification only points at the tray entry, where they decide what to do.
fn notify_once(app: &AppHandle, version: &str, installable: bool) {
    {
        let state = app.state::<UpdateState>();
        let mut last = state.notified.lock().unwrap();
        if last.as_deref() == Some(version) {
            return;
        }
        *last = Some(version.to_string());
    }
    let action = if installable || can_download_package() { "install" } else { "download" };
    let shown = app
        .notification()
        .builder()
        .title("ExcaliShot update available")
        .body(format!(
            "Version {version} is ready to {action}. Open ExcaliShot from the tray menu and choose \"Update available\"."
        ))
        .show();
    if let Err(e) = shown {
        eprintln!("could not show the update notification: {e}");
    }
}

/// Ask the update server whether a newer release exists, and record the answer.
/// `notify` is true for background checks (they should tell the user); a check the user asked
/// for already shows its result in the window.
pub async fn check(app: &AppHandle, notify: bool) {
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
                can_download: can_download_package(),
            };
            let version = update.version.clone();
            *state.pending.lock().unwrap() = Some(update);
            set_status(app, status);
            if notify {
                notify_once(app, &version, can_install());
            }
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
    check(&app, false).await;
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

/// `.deb` installs: fetch the new package and its signature, verify, save to Downloads and open
/// it with the system installer. The package is never opened if the signature does not match.
#[tauri::command]
pub async fn download_package(app: AppHandle) -> Result<(), String> {
    let version = match app.state::<UpdateState>().status.lock().unwrap().clone() {
        UpdateStatus::Available { version, .. } => version,
        _ => return Err("no update is available".into()),
    };
    if !can_download_package() {
        return Err("this installation cannot download packages itself".into());
    }
    if !is_plain_version(&version) {
        return Err(format!("refusing an unexpected version string: {version}"));
    }
    let arch = deb_arch().ok_or("unsupported CPU architecture")?;
    let (deb_url, sig_url) = package_urls(&version, arch);

    set_status(&app, UpdateStatus::Downloading { version: version.clone() });
    let result: Result<PathBuf, String> = async {
        let pubkey = updater_pubkey(&app)?;
        let deb = fetch(&deb_url).await?;
        let sig = String::from_utf8(fetch(&sig_url).await?).map_err(err)?;
        verify_signature(&pubkey, &sig, &deb).map_err(|e| format!("signature check failed: {e}"))?;

        let dir = app.path().download_dir().map_err(err)?;
        std::fs::create_dir_all(&dir).map_err(err)?;
        let path = dir.join(format!("ExcaliShot_{version}_{arch}.deb"));
        std::fs::write(&path, &deb).map_err(err)?;
        Ok(path)
    }
    .await;

    match result {
        Ok(path) => {
            app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(err)?;
            set_status(&app, UpdateStatus::Downloaded { version, path: path.display().to_string() });
            Ok(())
        }
        Err(message) => {
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
            json(&UpdateStatus::Available {
                version: "1.2.3".into(),
                notes: None,
                can_install: true,
                can_download: false
            }),
            serde_json::json!({
                "state": "available", "version": "1.2.3", "notes": null,
                "canInstall": true, "canDownload": false
            })
        );
        assert_eq!(
            json(&UpdateStatus::Downloaded { version: "1.2.3".into(), path: "/x.deb".into() }),
            serde_json::json!({ "state": "downloaded", "version": "1.2.3", "path": "/x.deb" })
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
            tray_text(&UpdateStatus::Available {
                version: "0.2.0".into(),
                notes: None,
                can_install: false,
                can_download: true
            }),
            "Update available: v0.2.0..."
        );
    }

    #[test]
    fn builds_release_asset_urls() {
        let (deb, sig) = package_urls("0.1.1", "amd64");
        assert_eq!(
            deb,
            "https://github.com/johmara/ExcaliShot/releases/download/v0.1.1/ExcaliShot_0.1.1_amd64.deb"
        );
        assert_eq!(sig, format!("{deb}.sig"));
    }

    #[test]
    fn only_plain_versions_are_accepted() {
        assert!(is_plain_version("0.1.1"));
        assert!(is_plain_version("1.2.3-rc.4"));
        assert!(!is_plain_version(""));
        assert!(!is_plain_version("1.0/../../etc"));
        assert!(!is_plain_version("1.0 && rm"));
        assert!(!is_plain_version(&"1".repeat(41)));
    }

    #[test]
    fn rejects_malformed_keys_and_signatures() {
        assert!(verify_signature("not base64 !!", "also not", b"data").is_err());
        assert!(verify_signature("", "", b"data").is_err());
    }

    /// The public key shipped in tauri.conf.json must be a decodable minisign key, or no update
    /// (including a downloaded .deb) could ever be verified.
    #[test]
    fn shipped_public_key_is_a_valid_minisign_key() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let pubkey = conf["plugins"]["updater"]["pubkey"].as_str().expect("pubkey in tauri.conf.json");
        let text = String::from_utf8(STANDARD.decode(pubkey).unwrap()).unwrap();
        assert!(minisign_verify::PublicKey::decode(&text).is_ok());
        // ...and a signature that does not belong to the data is refused.
        assert!(verify_signature(pubkey, "AAAA", b"data").is_err());
    }
}
