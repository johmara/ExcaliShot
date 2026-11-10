# ExcaliShot: Design

## Context

A cross-platform screenshot tool (Ubuntu/GNOME is the daily driver, plus other Linux distros, macOS, and Windows) with this core loop: **capture, annotate with Excalidraw arrows and boxes, then copy into chat or save to disk.** Flameshot was liked "when it worked", but on Wayland it captured every monitor at once and its region select misbehaved. **Getting Wayland right on GNOME is the main success criterion.**

## Goals
- Region selection capture and fullscreen capture.
- Capture only the monitor the mouse is on.
- Annotate with Excalidraw before saving.
- Copy to clipboard before or instead of saving (copy closes the editor).
- Save to disk, which matters as much as copy.
- User-configurable global shortcuts.
- Runs on Ubuntu 26.04 GNOME (Wayland), other Linux distros (X11 and Wayland), macOS, and Windows.

## Non-goals for v1
- Drawing directly on the frozen screen, Flameshot-style. Planned as a later **setting**; v1 uses a resizable editor window.
- Upload or sharing services, OCR, video, and scrolling capture.

## Anti-goals
- Capturing all monitors when the user asked for one.
- A region select that glitches, lands offset, or uses the wrong scale on Wayland or HiDPI.
- Shortcuts that silently don't fire on GNOME Wayland.

## Decisions
| Decision | Source |
|---|---|
| Rust core and React UI (Excalidraw needs React) | User |
| Resizable editor window; the overlay editor is a later setting | User |
| Copy puts the PNG on the clipboard and closes the editor | User |
| Save writes a timestamped PNG to a configurable folder (default `~/Pictures/Screenshots`, OS-equivalent elsewhere) and closes; Ctrl+Shift+S opens Save As | User |
| Tray icon and optional autostart on login | User |
| **Tauri v2**, React + TypeScript + Vite, `@excalidraw/excalidraw` | My call |
| "Fullscreen" means all monitors stitched together; "Current monitor" is a separate action | My call |
| Region select happens on a frozen overlay **before** the editor opens; the editor gets only the crop | My call |
| The screenshot is a locked background image element in Excalidraw; export uses `exportToBlob`, no padding, at native resolution | My call |
| PNG only in v1 | My call |

## Architecture

```
src-tauri/ (Rust)
  main.rs / lib.rs     app setup, tray, single-instance, CLI arg handling
  capture/mod.rs       trait CaptureBackend { capture_all() -> Frame + monitor layout }
  capture/portal.rs    Wayland: xdg-desktop-portal Screenshot via `ashpd` (non-interactive)
  capture/native.rs    X11 / Windows / macOS via `xcap`
  monitors.rs          monitor geometry and scale; crop helpers (logical to physical pixels)
  shortcuts/mod.rs     trait ShortcutBackend
  shortcuts/portal.rs  Wayland: GlobalShortcuts portal via `ashpd` (GNOME 48+, KDE)
  shortcuts/native.rs  tauri-plugin-global-shortcut (X11 / Windows / macOS)
  clipboard.rs         long-lived `arboard::Clipboard` held in app state so clipboard
                       ownership survives after the editor window closes
  settings.rs          JSON in the config dir (shortcuts, save folder, autostart)
  commands.rs          get_capture_png, copy_png, save_png, save_png_as, settings CRUD
src/ (React)
  overlay/   frozen-screen region selector (one fullscreen window per monitor)
  editor/    Excalidraw host: Copy / Save (Ctrl+S) / Save As (Ctrl+Shift+S)
  settings/  shortcut recorder, save folder picker, autostart toggle
```

### Capture flow
1. A trigger (global shortcut, tray menu, or CLI `excalishot capture region|monitor|full`) reaches the running instance via `tauri-plugin-single-instance`.
2. Capture the full desktop once: portal on Wayland, `xcap` elsewhere.
3. Branch on mode:
   - **full**: open the editor with the stitched image.
   - **monitor**: find the monitor under the pointer, crop to it, open the editor.
   - **region**: open a fullscreen overlay on **each** monitor showing its frozen slice. The user drags a rectangle (Esc cancels); crop and open the editor.
4. Editor window size is the image fit within 90% of the monitor, centered.

### Wayland-specific handling (the core risk)
- **Screen capture:** Screenshot portal with `interactive=false`. Returns one PNG of all outputs, cropped using monitor geometry and scale.
- **Mouse position:** Wayland exposes no global cursor position. Map a fullscreen overlay per output; the overlay that receives the first pointer event identifies the active monitor. Fallback: monitor with the focused window, then primary. **Spike #1.**
- **Global shortcuts:** GlobalShortcuts portal. Fallback, always documented: the CLI command bound as a GNOME custom shortcut.
- **Clipboard:** `arboard` keeps ownership through the long-lived process. Verify paste after the editor closes on GNOME Wayland. **Spike #2.**
- **Scale:** crop in physical pixels, derived from portal image size divided by logical desktop size. Test 100% and fractional scaling with mixed-DPI monitors.

### Other platforms
- macOS: Screen Recording permission (explain in UI); cursor position from Tauri `cursor_position()`.
- Windows/X11: `xcap`, `cursor_position()`, tauri-plugin-global-shortcut.

## Spike results (2026-11-09, Ubuntu 26.04 GNOME Wayland, 3 monitors, all scale 1)
- **Portal capture works** (`ashpd` 0.14, features `tokio,screenshot,global_shortcuts`). One PNG of the whole desktop; its size equals the bounding box of Tauri's monitor rectangles (5400x1966).
- **The portal writes the PNG to `~/Pictures/Screenshot-N.png` itself.** The app must read it and delete it immediately.
- **`cursor_position()` returns (0, 0) on Wayland.** Confirmed useless.
- **Monitor under the mouse via per-monitor overlays works**: one fullscreen undecorated window per monitor (GTK `fullscreen_on_monitor`, same index order as Tauri `available_monitors()`); the overlay under the resting pointer receives pointer events without movement.
- **Clipboard works** via both `arboard` (state-held, `wayland-data-control`) and the webview `navigator.clipboard.write`, pasted into Teams, and survives hiding the window. Decision: arboard primary, webview fallback. Not yet tested: a native-Wayland paste target.
- Not covered: fractional scaling and mixed-DPI. Crop math therefore derives its scale from the ratio of image size to monitor-bounding-box size, and is unit-tested.

## Implementation steps
0. Save this design to `docs/superpowers/specs/`.
1. Scaffold Tauri v2 + React + TS + Vite; add Excalidraw; tray icon with Capture Region / Monitor / Full / Settings / Quit.
2. **Spike: Wayland capture, monitor detection, and clipboard** on GNOME 26.04. Adjust the design if anything fails.
3. Capture backends behind the trait, plus monitor geometry and crop helpers with unit tests for scale math.
4. Region overlay windows (multi-monitor, HiDPI, Esc cancel, selection rectangle with dimensions).
5. Editor: load bytes, locked background image, export, Copy (close) / Save (auto folder, close) / Save As.
6. Shortcut backends, Settings UI with a shortcut recorder, persisted settings, autostart.
7. CLI subcommands and single-instance forwarding; README documents the GNOME custom-shortcut fallback.
8. Packaging: `.deb` + AppImage, `.dmg`, `.msi`; GitHub Actions build matrix.

## Verification
- Rust unit tests: crop and scale math (mixed DPI, negative monitor offsets), settings round-trip, filename generation.
- Manual on Ubuntu 26.04 GNOME Wayland, 2 monitors:
  - Region: overlay on both monitors, crop matches the drag at 100% and 125%/150%.
  - Monitor: with the mouse on B, only B is captured; repeat for A.
  - Full: both monitors stitched.
  - Annotate, Copy, editor closes, paste into a browser or chat app shows the annotated image.
  - Save writes a PNG in `~/Pictures/Screenshots`; Save As opens a dialog.
  - Shortcut changes apply without restart; autostart works after re-login.
- Smoke test on X11, Windows, macOS (CI builds plus manual where hardware is available).
