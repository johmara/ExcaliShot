# ExcaliShot

A cross-platform screenshot tool that lets you sketch on the capture with [Excalidraw](https://excalidraw.com) before you copy or save it.

Capture, draw arrows and boxes on it, paste it into a chat.

## Features

- **Region**, **current monitor** (the one your mouse is on) and **all monitors** capture, started instantly from global shortcuts, the tray menu or the command line.
- The capture opens in a resizable Excalidraw editor, fitted to the window, with the screenshot as a locked background. Draw, add text, then **Copy** or **Save**. The editor works offline.
- Copy puts the annotated PNG on the clipboard and closes the editor. Save writes a timestamped PNG to `~/Pictures/Screenshots`.
- Built for Wayland as well as X11. It does not capture every monitor when you asked for one.
- Lives in the tray: capture entries that show their shortcut, a "Start on login" toggle, settings, and update checks.
- Signed in-app updates with a notification when a new version is available (see [Updates](#updates)).

## Status

| Platform | Capture | Global shortcuts |
|---|---|---|
| Ubuntu / GNOME (Wayland) | Working (xdg-desktop-portal) | Working (portal GlobalShortcuts, or a desktop keybinding calling the CLI) |
| Other Linux (KDE, X11, ...) | Portal-based, untested | Portal-based, untested |
| macOS | Implemented (`xcap`), built by CI, **not tested on a real Mac** | Implemented, shortcuts editable in the settings window, **untested** |
| Windows | Implemented (`xcap`), built by CI, **not tested on a real PC** | Implemented, shortcuts editable in the settings window, **untested** |

Tested so far on Ubuntu 26.04 (GNOME, Wayland, three monitors). Contributions and test reports for the other platforms are welcome, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Install

Download the package for your system from the [Releases](https://github.com/johmara/ExcaliShot/releases) page. Entries marked "Pre-release" are release candidates built from `main` (they are deleted when the next real release is published); the one marked "Latest" is the current release.

| System | Download | Notes |
|---|---|---|
| Ubuntu, Debian and relatives | `ExcaliShot_<version>_amd64.deb` | `sudo apt install ./ExcaliShot_<version>_amd64.deb`. Remove it with `sudo apt remove excali-shot`. |
| Any Linux | `ExcaliShot_<version>_amd64.AppImage` | `chmod +x` and run. Updates itself in place. Needs FUSE 2 (`sudo apt install libfuse2t64` on recent Ubuntu). |
| Fedora and relatives | `.rpm` | |
| macOS | `.dmg` (universal) | |
| Windows | `.msi` or `.exe` | |

Ignore the `.sig` files and `latest.json`: they belong to the updater.

#### First launch on macOS and Windows

The packages are not signed with an Apple or Microsoft certificate yet (that costs money and is on the roadmap), so both systems warn the first time you open the app. The app itself is built from this repository by GitHub Actions, and in-app updates are verified with ExcaliShot's own signing key.

- **Windows:** in the SmartScreen dialog click **More info**, then **Run anyway**.
- **macOS:** open the app once, dismiss the warning, then go to **System Settings > Privacy & Security**, scroll to the message about ExcaliShot and click **Open Anyway**. On older macOS versions, right-click the app and choose **Open** instead. If macOS says the app is damaged, run `xattr -cr /Applications/ExcaliShot.app` and open it again.

Installing or removing the Linux package stops a running copy first, so the new version is the one that starts next.

### Updates

ExcaliShot checks for a new release shortly after it starts and then once a day, and from **Check for updates...** in the tray menu or the settings window. When a newer version exists you get a desktop notification once per version, and the tray entry changes to "Update available". **Nothing is downloaded or installed until you choose to.** Only real releases are offered, never the pre-releases.

Every update is verified against a signature made with ExcaliShot's release key before it is installed or opened.

- **macOS, Windows and the Linux AppImage:** one click on "Install ... and restart".
- **Linux `.deb` (Ubuntu, Debian):** one click on "Download and install". The app downloads the new `.deb`, checks its signature, saves it to your Downloads folder and opens the system installer, which asks for your password. Finish the installation there and start ExcaliShot again.
- **Linux `.rpm` and anything else:** the settings window links to the release page.

## Usage

Start the app. It lives in the system tray (on GNOME you need the AppIndicator extension, which Ubuntu ships by default). The tray menu has the three capture actions, a **Start on login** checkbox, **Settings...**, **Change shortcuts...**, and **Check for updates...**.

Default shortcuts, which your desktop may change:

| Action | Shortcut |
|---|---|
| Capture region | `Ctrl+Shift+1` |
| Capture current monitor | `Ctrl+Shift+2` |
| Capture all monitors | `Ctrl+Shift+3` |

In the editor: `Ctrl+Enter` (or `Ctrl+C` with nothing selected) copies, `Ctrl+S` saves, `Esc` cancels a region selection.

### Changing the shortcuts

- **macOS and Windows:** open **Settings...** from the tray, click **Change** next to an action, and press the new combination.
- **GNOME:** the desktop owns the shortcuts and shows the current keys in the settings window. Newer GNOME versions list them under **Settings > Apps > ExcaliShot**. If yours does not, bind the command line instead, which works on every desktop: **Settings > Keyboard > Custom Shortcuts**, with one entry per action and a command like:

```
excalishot-app capture region
excalishot-app capture monitor
excalishot-app capture full
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
npm run tauri build    # produce installable bundles (needs the signing key, see CONTRIBUTING.md)
```

Background update checks and notifications are disabled in dev builds; use the "Check for updates" button to try the update flow.

### Global shortcuts during development (GNOME)

The shortcuts portal needs an installed `.desktop` file that matches the app ID, `se.johmara.excalishot`. The installed package provides one. Without it, a `tauri dev` binary can create its own that points at the debug binary:

```bash
printf '[Desktop Entry]\nType=Application\nName=ExcaliShot\nExec=%s/src-tauri/target/debug/excalishot-app\nIcon=applets-screenshooter\nTerminal=false\nCategories=Utility;\nStartupWMClass=excalishot-app\n' "$PWD" > ~/.local/share/applications/se.johmara.excalishot.desktop
```

**Delete that file when you are done developing.** A user-level launcher shadows the packaged one with the same ID: the installed app then starts the debug binary (which fails with "Could not connect to localhost") and shows the stock icon in the dock.

### Project layout

```
src-tauri/src/
  capture.rs     desktop capture backends (portal on Linux, xcap on macOS and Windows), PNG helpers
  flow.rs        capture flow: overlays, editor window, copy and save commands
  monitors.rs    monitor geometry and crop/scale math (unit-tested)
  stitch.rs      matching and stitching per-monitor shots on macOS and Windows (unit-tested)
  shortcuts.rs   global shortcuts (xdg-desktop-portal on Linux, global-shortcut plugin elsewhere)
  settings.rs    stored shortcut choices for macOS and Windows
  updater.rs     update checks, notifications, installing and the .deb download
  lib.rs         app setup, tray menu, CLI handling, single instance
src/
  overlay/       frozen-screen region selector, one preloaded window per monitor
  editor/        Excalidraw editor with Copy and Save
  settings/      settings window (Excalidraw-styled)
packaging/linux/ launcher file, package scripts
design/          logo sources
docs/superpowers/specs/   design document and Wayland spike results
.github/workflows/        CI, pre-releases and releases
```

### How it works on Wayland

Wayland deliberately hides the cursor position and blocks screen grabbing, so ExcaliShot does not use the usual tricks:

- It captures through the **Screenshot portal** (one image of the whole desktop) and crops it itself.
- It finds the monitor under your mouse by showing one fullscreen overlay per monitor. The overlay that receives the first pointer event is the answer. The overlays are created hidden at startup and only shown once their image has rendered, so a capture starts instantly.
- Shortcuts go through the **GlobalShortcuts portal**, with the CLI as the fallback.

The design document in [`docs/superpowers/specs/`](docs/superpowers/specs/2026-11-09-excalishot-design.md) has the details and the results of the spikes that settled these choices.

## Releases

Pushes to `main` publish a pre-release (`vX.Y.Z-rc.N`); pushing a version tag publishes the real release and deletes the pre-releases. All of it is automated, see [CONTRIBUTING.md](CONTRIBUTING.md#releases).

## Roadmap

- Test and fix macOS and Windows on real hardware (overlay windows, Screen Recording permission, the shortcut recorder).
- Platform code signing and notarization, so macOS and Windows stop showing warnings.
- Self-downloading updates for `.rpm` installs.
- An option to annotate directly on the frozen screen, as Flameshot does.

## License

[MIT](LICENSE). The Excalifont and Assistant fonts in `public/fonts` are distributed with Excalidraw under the SIL Open Font License; see their upstream projects for the license text.
