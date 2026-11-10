# ExcaliShot

A cross-platform screenshot tool that lets you sketch on the capture with [Excalidraw](https://excalidraw.com) before you copy or save it.

Capture, draw arrows and boxes on it, paste it into a chat.

## Features

- **Region**, **current monitor** (the one your mouse is on) and **all monitors** capture.
- The capture opens in a resizable Excalidraw editor, with the screenshot as a locked background. Draw, add text, then **Copy** or **Save**.
- Copy puts the annotated PNG on the clipboard and closes the editor. Save writes a timestamped PNG to `~/Pictures/Screenshots`.
- Global shortcuts, a tray menu, and a `capture` command-line interface.
- Built for Wayland as well as X11. It does not capture every monitor when you asked for one.

## Status

| Platform | Capture | Global shortcuts |
|---|---|---|
| Ubuntu / GNOME (Wayland) | Working (xdg-desktop-portal) | Working (portal GlobalShortcuts, or a desktop keybinding calling the CLI) |
| Other Linux (KDE, X11, ...) | Portal-based, untested | Portal-based, untested |
| macOS | Not implemented yet | Not implemented yet |
| Windows | Not implemented yet | Not implemented yet |

Tested so far on Ubuntu 26.04 (GNOME, Wayland, three monitors). Contributions for the other platforms are welcome, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Install

Download the package for your system from the [Releases](https://github.com/johmara/ExcaliShot/releases) page. Entries marked "Pre-release" are release candidates built from `main`; the one marked "Latest" is the current release.

The packages are not code-signed yet, so macOS and Windows show a warning the first time you open the app.

### Updates

ExcaliShot checks for a new release shortly after it starts, and from **Check for updates...** in the tray menu or the settings window. Only real releases are offered, never the pre-releases. Updates are verified against a signature before they are installed.

- **macOS, Windows and the Linux AppImage** install the update and restart with one click.
- **Linux `.deb` and `.rpm` installs** cannot replace themselves: the settings window offers a download link, and you install the new package over the old one.

## Usage

Start the app. It lives in the system tray (on GNOME you need the AppIndicator extension, which Ubuntu ships by default).

Default shortcuts, which your desktop may change:

| Action | Shortcut |
|---|---|
| Capture region | `Ctrl+Shift+1` |
| Capture current monitor | `Ctrl+Shift+2` |
| Capture all monitors | `Ctrl+Shift+3` |

In the editor: `Ctrl+Enter` (or `Ctrl+C` with nothing selected) copies, `Ctrl+S` saves, `Esc` cancels a region selection.

### Changing the shortcuts

On GNOME the shortcuts are owned by the desktop. Newer versions list them under **Settings > Apps > ExcaliShot**. If yours does not, bind the command line instead, which works on every desktop:

**Settings > Keyboard > Custom Shortcuts**, then add one entry per action with a command like:

```
excalishot capture region
excalishot capture monitor
excalishot capture full
```

A second invocation hands the request to the running instance and exits straight away.

## Development

Prerequisites: [Rust](https://rustup.rs), Node.js 20 or newer, and the [Tauri system dependencies](https://tauri.app/start/prerequisites/). On Ubuntu:

```bash
sudo apt install -y build-essential curl wget file libwebkit2gtk-4.1-dev libxdo-dev \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev libpipewire-0.3-dev \
  libgbm-dev libxcb1-dev libxrandr-dev
```

```bash
npm install
npm run tauri dev      # run the app
cd src-tauri && cargo test   # Rust unit tests
npm run build          # type-check and build the frontend
npm run tauri build    # produce installable bundles
```

### Global shortcuts during development (GNOME)

The shortcuts portal needs the app to have an installed `.desktop` file that matches its ID, `se.johmara.excalishot`. A `tauri dev` binary has none, so create one that points at the debug binary:

```bash
printf '[Desktop Entry]\nType=Application\nName=ExcaliShot\nExec=%s/src-tauri/target/debug/excalishot-app\nIcon=applets-screenshooter\nTerminal=false\nCategories=Utility;\nStartupWMClass=excalishot-app\n' "$PWD" > ~/.local/share/applications/se.johmara.excalishot.desktop
```

The same file is why the GNOME dock shows a generic icon in dev builds. Installed bundles ship their own `.desktop` file.

### Project layout

```
src-tauri/src/
  capture.rs     desktop capture backends (portal on Linux) and PNG helpers
  flow.rs        capture flow: overlays, editor window, copy and save commands
  monitors.rs    monitor geometry and crop/scale math (unit-tested)
  shortcuts.rs   global shortcuts (xdg-desktop-portal on Linux)
  lib.rs         app setup, tray menu, CLI handling, single instance
src/
  overlay/       frozen-screen region selector, one window per monitor
  editor/        Excalidraw editor with Copy and Save
  settings/      settings window
design/          logo sources
docs/superpowers/specs/   design document and Wayland spike results
```

### How it works on Wayland

Wayland deliberately hides the cursor position and blocks screen grabbing, so ExcaliShot does not use the usual tricks:

- It captures through the **Screenshot portal** (one image of the whole desktop) and crops it itself.
- It finds the monitor under your mouse by opening one fullscreen overlay per monitor. The overlay that receives the first pointer event is the answer.
- Shortcuts go through the **GlobalShortcuts portal**, with the CLI as the fallback.

The design document in [`docs/superpowers/specs/`](docs/superpowers/specs/2026-11-09-excalishot-design.md) has the details and the results of the spikes that settled these choices.

## Roadmap

In priority order:

1. Faster region select: overlay windows are preloaded hidden, so a capture starts immediately (in testing).
2. Offline Excalidraw fonts in the editor.
3. macOS and Windows capture (`xcap`) and shortcuts (`tauri-plugin-global-shortcut`), with a shortcut editor in the settings window, plus a CI build matrix for Linux, macOS and Windows.
4. Packaging: `.deb`, AppImage, `.dmg` and `.msi`.

Later: an option to annotate directly on the frozen screen, as Flameshot does.

## License

[MIT](LICENSE). The Excalifont and Assistant fonts in `public/fonts` are distributed with Excalidraw under the SIL Open Font License; see their upstream projects for the license text.
