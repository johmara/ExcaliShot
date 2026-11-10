mod capture;
mod flow;
mod monitors;
mod settings;
mod shortcuts;
mod stitch;
mod updater;

use flow::{AppState, Mode};
use tauri::{
    AppHandle, Manager, RunEvent, Wry, WindowEvent,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

/// Handles to the tray menu entries that change at runtime.
struct TrayItems {
    region: MenuItem<Wry>,
    monitor: MenuItem<Wry>,
    full: MenuItem<Wry>,
    autostart: CheckMenuItem<Wry>,
    update: MenuItem<Wry>,
}

/// `excalishot capture region|monitor|full`
fn parse_cli(args: &[String]) -> Option<Mode> {
    let i = args.iter().position(|a| a == "capture")?;
    Mode::parse(args.get(i + 1)?.as_str())
}

fn trigger(app: &AppHandle, mode: Mode) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = flow::start(app, mode).await {
            eprintln!("capture failed: {e}");
        }
    });
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Put the current shortcut next to each capture entry, e.g. "Capture region  (Ctrl+Shift+1)".
pub(crate) fn refresh_tray(app: &AppHandle) {
    let Some(items) = app.try_state::<TrayItems>() else { return };
    let status = app.state::<shortcuts::StatusState>().0.lock().unwrap().clone();
    let label = |base: &str, id: &str| match status.trigger_for(id) {
        Some(t) => format!("{base}  ({t})"),
        None => base.to_string(),
    };
    let _ = items.region.set_text(label("Capture region", "region"));
    let _ = items.monitor.set_text(label("Capture current monitor", "monitor"));
    let _ = items.full.set_text(label("Capture all monitors", "full"));
}

#[tauri::command]
fn autostart_get(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn autostart_set(app: AppHandle, enabled: bool) -> Result<(), String> {
    let launch = app.autolaunch();
    if enabled { launch.enable() } else { launch.disable() }.map_err(|e| e.to_string())?;
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.autostart.set_checked(enabled); // keep the tray checkbox in sync
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        // Must be first. A second launch (e.g. `excalishot capture region` bound to a desktop
        // keybinding) forwards its arguments here and exits.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| match parse_cli(&argv) {
            Some(mode) => trigger(app, mode),
            None => show_main(app),
        }));
    // Linux gets its shortcuts from the xdg-desktop-portal instead (see shortcuts.rs).
    #[cfg(not(target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_global_shortcut::Builder::new().build());

    builder
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_opener::init())
        // Needs `plugins.updater` (public key and endpoint) in tauri.conf.json.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .manage(AppState::default())
        .manage(shortcuts::PortalState::default())
        .manage(shortcuts::StatusState::default())
        .manage(shortcuts::SettingsState::default())
        .manage(updater::UpdateState::default())
        .setup(|app| {
            let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
            let region = MenuItem::with_id(app, "region", "Capture region", true, None::<&str>)?;
            let monitor = MenuItem::with_id(app, "monitor", "Capture current monitor", true, None::<&str>)?;
            let full = MenuItem::with_id(app, "full", "Capture all monitors", true, None::<&str>)?;
            let autostart = CheckMenuItem::with_id(app, "autostart", "Start on login", true, autostart_on, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
            let shortcuts_item = MenuItem::with_id(app, "shortcuts", "Change shortcuts...", true, None::<&str>)?;
            let update_item = MenuItem::with_id(app, "update", "Check for updates...", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(
                app,
                &[
                    &region, &monitor, &full, &sep1, &autostart, &settings_item, &shortcuts_item,
                    &update_item, &sep2, &quit,
                ],
            )?;
            app.manage(TrayItems { region, monitor, full, autostart, update: update_item });

            TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("window icon").clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "region" => trigger(app, Mode::Region),
                    "monitor" => trigger(app, Mode::Monitor),
                    "full" => trigger(app, Mode::Full),
                    "settings" | "shortcuts" => show_main(app),
                    "update" => {
                        // Check, then open the settings window where the result (and the install
                        // button) is shown.
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            updater::check(&app, false).await;
                            show_main(&app);
                        });
                    }
                    "autostart" => {
                        // The check item has already toggled itself; make the system match it.
                        let items = app.state::<TrayItems>();
                        let want = items.autostart.is_checked().unwrap_or(false);
                        let launch = app.autolaunch();
                        let res = if want { launch.enable() } else { launch.disable() };
                        if let Err(e) = res {
                            eprintln!("autostart change failed: {e}");
                            let _ = items.autostart.set_checked(!want);
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // Background update check shortly after startup and then once a day (not in dev
            // builds), with a notification when a new version appears. A plain thread, so the
            // waits do not need an extra async-timer dependency.
            if !cfg!(debug_assertions) {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(15));
                    loop {
                        tauri::async_runtime::block_on(updater::check(&handle, true));
                        std::thread::sleep(std::time::Duration::from_secs(24 * 60 * 60));
                    }
                });
            }

            // Hidden overlay windows, ready before the first capture.
            flow::preload_overlays(app.handle());

            // Bind global shortcuts last: the tray labels are refreshed when binding finishes.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move { shortcuts::init(handle).await });

            // First launch: honour a CLI capture request, otherwise stay in the tray
            // (dev builds and `--show` open the settings window).
            let args: Vec<String> = std::env::args().collect();
            if let Some(mode) = parse_cli(&args) {
                trigger(app.handle(), mode);
            } else if cfg!(debug_assertions) || args.iter().any(|a| a == "--show") {
                show_main(app.handle());
            }
            Ok(())
        })
        // Closing the settings window hides it; the app keeps running in the tray.
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            flow::start_capture,
            flow::overlay_mode,
            flow::overlay_slice,
            flow::overlay_ready,
            flow::overlay_pick,
            flow::overlay_select,
            flow::cancel_capture,
            flow::editor_image,
            flow::copy_png,
            flow::save_png,
            shortcuts::shortcut_status,
            shortcuts::configure_shortcuts,
            shortcuts::set_shortcut,
            autostart_get,
            autostart_set,
            updater::update_status,
            updater::check_for_update,
            updater::install_update,
            updater::download_package,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // Closing the last window must not quit a tray app; only an explicit exit (code Some) does.
            if let RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_capture_modes() {
        assert_eq!(parse_cli(&args(&["excalishot", "capture", "region"])), Some(Mode::Region));
        assert_eq!(parse_cli(&args(&["x", "capture", "monitor"])), Some(Mode::Monitor));
        assert_eq!(parse_cli(&args(&["x", "--show", "capture", "full"])), Some(Mode::Full));
    }

    #[test]
    fn rejects_everything_else() {
        assert_eq!(parse_cli(&args(&["excalishot"])), None);
        assert_eq!(parse_cli(&args(&["excalishot", "capture"])), None);
        assert_eq!(parse_cli(&args(&["excalishot", "capture", "window"])), None);
        assert_eq!(parse_cli(&args(&["excalishot", "--show"])), None);
    }
}
