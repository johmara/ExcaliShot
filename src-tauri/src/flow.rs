//! Capture flow: trigger -> grab desktop -> (overlays) -> editor -> copy/save.

use std::sync::Mutex;

use image::DynamicImage;
use tauri::{
    AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder,
    ipc::{InvokeBody, Request, Response},
};

use crate::{
    capture,
    monitors::{self, Rect},
};

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Full,
    Monitor,
    Region,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        match s {
            "full" => Some(Mode::Full),
            "monitor" => Some(Mode::Monitor),
            "region" => Some(Mode::Region),
            _ => None,
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Mode::Full => "full",
            Mode::Monitor => "monitor",
            Mode::Region => "region",
        }
    }
}

struct Session {
    desktop: DynamicImage,
    /// Monitor rectangles in Tauri coordinates, same order as the overlay indices.
    monitors: Vec<Rect>,
    bbox: Rect,
    mode: Mode,
}

#[derive(Default)]
pub struct AppState {
    session: Mutex<Option<Session>>,
    /// The PNG the editor is annotating.
    result: Mutex<Option<Vec<u8>>>,
    /// Long-lived so clipboard ownership survives the editor window closing.
    clipboard: Mutex<Option<arboard::Clipboard>>,
}

fn close_windows(app: &AppHandle, prefix: &str) {
    for (label, win) in app.webview_windows() {
        if label.starts_with(prefix) {
            let _ = win.destroy();
        }
    }
}

/// Entry point for every trigger (tray, shortcut, CLI, launcher button).
pub async fn start(app: AppHandle, mode: Mode) -> Result<(), String> {
    close_windows(&app, "ov");
    close_windows(&app, "editor");

    let desktop = capture::grab_desktop().await?;
    let mons: Vec<Rect> = app
        .available_monitors()
        .map_err(err)?
        .iter()
        .map(|m| Rect {
            x: m.position().x,
            y: m.position().y,
            w: m.size().width,
            h: m.size().height,
        })
        .collect();
    let bbox = monitors::bounding_box(&mons).ok_or("no monitors found")?;
    let state = app.state::<AppState>();

    if mode == Mode::Full {
        let png = capture::encode_png(&desktop, false)?;
        *state.result.lock().unwrap() = Some(png);
        return open_editor(&app, desktop.width(), desktop.height());
    }

    *state.session.lock().unwrap() = Some(Session { desktop, monitors: mons.clone(), bbox, mode });
    open_overlays(&app, &mons, mode)
}

fn open_overlays(app: &AppHandle, mons: &[Rect], mode: Mode) -> Result<(), String> {
    for (i, m) in mons.iter().enumerate() {
        let win = WebviewWindowBuilder::new(
            app,
            format!("ov{i}"),
            WebviewUrl::App(format!("index.html?overlay={i}&mode={}", mode.as_str()).into()),
        )
        .decorations(false)
        .always_on_top(true)
        .focused(true)
        .build()
        .map_err(err)?;

        #[cfg(target_os = "linux")]
        {
            // Wayland ignores client-side positioning; ask GTK to fullscreen on monitor `i`.
            let _ = m;
            let w = win.clone();
            app.run_on_main_thread(move || {
                use gtk::prelude::*;
                if let Ok(gw) = w.gtk_window() {
                    if let Some(screen) = GtkWindowExt::screen(&gw) {
                        gw.fullscreen_on_monitor(&screen, i as i32);
                    }
                }
            })
            .map_err(err)?;
        }
        // TODO(step 8): verify on macOS/Windows.
        #[cfg(not(target_os = "linux"))]
        {
            let _ = win.set_position(tauri::PhysicalPosition::new(m.x, m.y));
            let _ = win.set_size(tauri::PhysicalSize::new(m.w, m.h));
            let _ = win.set_fullscreen(true);
        }
    }
    Ok(())
}

fn open_editor(app: &AppHandle, img_w: u32, img_h: u32) -> Result<(), String> {
    close_windows(app, "editor");
    let (mw, mh) = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| (m.size().width as f64 / m.scale_factor(), m.size().height as f64 / m.scale_factor()))
        .unwrap_or((1280.0, 800.0));
    let w = (img_w as f64).min(mw * 0.9).max(520.0);
    let h = (img_h as f64 + 80.0).min(mh * 0.9).max(360.0);
    WebviewWindowBuilder::new(app, "editor", WebviewUrl::App("index.html?editor=1".into()))
        .title("ExcaliShot")
        .inner_size(w, h)
        .resizable(true)
        .focused(true)
        .build()
        .map_err(err)?;
    Ok(())
}

/// Crop `rect` (desktop-image pixels) out of the session, close overlays, open the editor.
fn finish(app: &AppHandle, rect: Rect) -> Result<(), String> {
    let state = app.state::<AppState>();
    let png = {
        let guard = state.session.lock().unwrap();
        let s = guard.as_ref().ok_or("no active capture")?;
        capture::crop_png(&s.desktop, rect, false)?
    };
    *state.session.lock().unwrap() = None;
    close_windows(app, "ov");
    *state.result.lock().unwrap() = Some(png);
    open_editor(app, rect.w, rect.h)
}

// ---- commands -------------------------------------------------------------

#[tauri::command]
pub async fn start_capture(app: AppHandle, mode: String) -> Result<(), String> {
    start(app, Mode::parse(&mode).ok_or("unknown mode")?).await
}

/// The frozen picture of one monitor, shown by that monitor's overlay.
#[tauri::command]
pub fn overlay_slice(state: State<'_, AppState>, index: usize) -> Result<Response, String> {
    let guard = state.session.lock().unwrap();
    let s = guard.as_ref().ok_or("no active capture")?;
    let m = *s.monitors.get(index).ok_or("bad monitor index")?;
    let r = monitors::monitor_in_image(m, s.bbox, s.desktop.width(), s.desktop.height());
    Ok(Response::new(capture::crop_png(&s.desktop, r, true)?))
}

/// Monitor mode: the overlay that first sees the pointer is the monitor to capture.
#[tauri::command]
pub fn overlay_pick(app: AppHandle, state: State<'_, AppState>, index: usize) -> Result<(), String> {
    let rect = {
        let guard = state.session.lock().unwrap();
        let s = guard.as_ref().ok_or("no active capture")?;
        if s.mode != Mode::Monitor {
            return Ok(());
        }
        let m = *s.monitors.get(index).ok_or("bad monitor index")?;
        monitors::monitor_in_image(m, s.bbox, s.desktop.width(), s.desktop.height())
    };
    finish(&app, rect)
}

/// Region mode: selection in the overlay's CSS pixels (signed w/h allowed).
#[tauri::command]
pub fn overlay_select(
    app: AppHandle,
    state: State<'_, AppState>,
    index: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    ow: f64,
    oh: f64,
) -> Result<(), String> {
    let rect = {
        let guard = state.session.lock().unwrap();
        let s = guard.as_ref().ok_or("no active capture")?;
        let m = *s.monitors.get(index).ok_or("bad monitor index")?;
        let mon = monitors::monitor_in_image(m, s.bbox, s.desktop.width(), s.desktop.height());
        monitors::selection_in_image(mon, ow, oh, (x, y, w, h))
    };
    match rect {
        Some(r) => finish(&app, r),
        None => Ok(()), // empty selection: keep waiting
    }
}

#[tauri::command]
pub fn cancel_capture(app: AppHandle, state: State<'_, AppState>) {
    *state.session.lock().unwrap() = None;
    close_windows(&app, "ov");
}

#[tauri::command]
pub fn editor_image(state: State<'_, AppState>) -> Result<Response, String> {
    let png = state.result.lock().unwrap().clone().ok_or("no image")?;
    Ok(Response::new(png))
}

fn body_bytes(req: &Request<'_>) -> Result<Vec<u8>, String> {
    match req.body() {
        InvokeBody::Raw(b) => Ok(b.clone()),
        _ => Err("expected raw PNG bytes".into()),
    }
}

/// Copy the (annotated) PNG to the clipboard, then close the editor.
#[tauri::command]
pub fn copy_png(app: AppHandle, state: State<'_, AppState>, request: Request<'_>) -> Result<(), String> {
    let img = image::load_from_memory(&body_bytes(&request)?).map_err(err)?.to_rgba8();
    let (w, h) = img.dimensions();
    {
        let mut slot = state.clipboard.lock().unwrap();
        if slot.is_none() {
            *slot = Some(arboard::Clipboard::new().map_err(err)?);
        }
        slot.as_mut()
            .unwrap()
            .set_image(arboard::ImageData {
                width: w as usize,
                height: h as usize,
                bytes: img.into_raw().into(),
            })
            .map_err(err)?;
    }
    close_windows(&app, "editor");
    Ok(())
}

/// Save the PNG as a timestamped file in ~/Pictures/Screenshots, then close the editor.
#[tauri::command]
pub fn save_png(app: AppHandle, request: Request<'_>) -> Result<String, String> {
    let bytes = body_bytes(&request)?;
    let dir = app.path().picture_dir().map_err(err)?.join("Screenshots"); // TODO: configurable (step 6)
    std::fs::create_dir_all(&dir).map_err(err)?;
    let stamp = chrono::Local::now().format("Screenshot_%Y-%m-%d_%H-%M-%S");
    let mut path = dir.join(format!("{stamp}.png"));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("{stamp}_{n}.png"));
        n += 1;
    }
    std::fs::write(&path, bytes).map_err(err)?;
    close_windows(&app, "editor");
    Ok(path.display().to_string())
}
