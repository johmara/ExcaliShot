# Annotate on the frozen screen ("Flameshot mode") - short design

Issue: #6. Date: 2026-11-16.

## Intent and why
Flameshot lets you draw right after dragging a region, with no separate window. The user wants that
capture-then-annotate flow in ExcaliShot, using Excalidraw's own toolbar (their call), not a custom
Flameshot-style one.

## Goals
- Setting "Annotate on the frozen screen", off by default.
- With it on, region selection leaves the overlay in place as an Excalidraw canvas: the frozen
  monitor is a locked background, the area outside the selection is dimmed, Excalidraw's normal
  toolbar is usable.
- Copy (`Ctrl+Enter`, `Ctrl+C` with nothing selected), Save (`Ctrl+S`) and Esc behave as in the editor window.
- Works with the per-monitor overlays on Wayland, HiDPI and multiple monitors.

## Non-goals / anti-goals
- No custom toolbar next to the selection (the toolbar sits at the top of the monitor; accepted).
- Monitor and full captures are unchanged (they still open the editor window).
- No cost for users with the setting off: no extra startup weight, no behavior change.
- No blank or white flash when the overlay changes mode.

## Approaches considered
- A. Excalidraw always mounted in every preloaded overlay: no window swap, but every user pays the startup cost.
- B. New fullscreen annotate window after selection: no cost when off, but a window swap risks a flash on Wayland.
- **C (chosen, user picked): lazy-load Excalidraw inside the existing overlay after selection.** No window swap, no cost when off.

## Design (my calls unless noted)
- Setting `annotate_on_screen: bool` (default false) in `settings.rs` with a checkbox in the settings window. It must work on Linux too, where settings.rs is currently unused (shortcuts are owned by the desktop), so loading and saving cannot stay behind the macOS/Windows-only path.
- On `overlay-arm` in region mode with the setting on, start preloading the Excalidraw chunk (`React.lazy`) so it is ready by the time the selection ends.
- On drag end, instead of `overlay_select`: tell Rust the selection is made and which overlay owns it; Rust hides the other overlays (not cancel), keeps the session. The owning overlay mounts Excalidraw with its slice as a locked image.
- Scale: image elements are in physical pixels, so the viewport zoom is set to css-size / physical-size to fit the monitor exactly; the selection is stored in scene units.
- Export: full-monitor scene PNG at native resolution, sent to Rust with the selection and overlay size; Rust maps it with `monitors::selection_in_image` logic (relative to the monitor slice) and crops with `capture::crop_png`, then copies or saves and hides overlays. Cropping happens in Rust so it reuses the tested math.
- Reuse the editor's export, key handling and message code by extracting them from `Editor.tsx` into a shared module rather than duplicating.
- Dim mask outside the selection is drawn as an overlay `div` above the canvas with `pointer-events: none`.
- Esc: Excalidraw consumes Esc for tool changes; first Esc with a tool active keeps Excalidraw's behavior, Esc with nothing in progress cancels the capture.

## Left to the builder
- Exact placement of the Copy/Save buttons in the overlay (the editor has a top bar; the overlay needs a floating pair near the selection or top corner).
- Tests: Rust unit tests for the new selection-to-crop mapping and the settings round trip with the new field; manual check on Wayland at 100% and 150%, two monitors.
