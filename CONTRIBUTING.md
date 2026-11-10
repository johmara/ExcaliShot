# Contributing to ExcaliShot

Thanks for helping. This is a small project, so the process is light.

## Getting set up

Follow the **Development** section of the [README](README.md) to install the prerequisites and run the app with `npm run tauri dev`.

## Before you open a pull request

Run these from the repository root and make sure they pass:

```bash
cd src-tauri && cargo test && cargo check && cd ..
npm run build
```

Also run `cargo fmt` and `cargo clippy` in `src-tauri` on the code you touched.

For anything that changes behaviour, test it by hand and say how in the pull request. The capture flow depends on the desktop, so "it compiles" is not enough. For capture, overlay or shortcut changes, please state the OS, desktop and session type (for example "Ubuntu 26.04, GNOME, Wayland, 2 monitors") and the monitor scaling you tried.

## Guidelines

- **Keep Wayland first-class.** Do not add code that depends on a global cursor position or on grabbing the screen directly; those do not work on Wayland. See "How it works on Wayland" in the README.
- **Put pure logic in testable functions** with unit tests, as in `src-tauri/src/monitors.rs`. Coordinate and scale math is where the bugs are.
- **Keep platform code behind `cfg`.** Linux, macOS and Windows backends sit behind the same functions (see `capture::grab_desktop` and `shortcuts.rs`). Unimplemented platforms return a clear error instead of panicking.
- **Do not leave stray files behind.** The Linux screenshot portal writes into `~/Pictures`; the app deletes it after reading it.
- **Treat screenshots as private.** Never send capture data anywhere or write it to disk except through the explicit Save action.
- Keep changes focused. One topic per pull request.

## Good first contributions

- macOS and Windows capture and shortcuts (see the Roadmap in the README). These need testing on real hardware.
- Reports from other Linux desktops (KDE Plasma, Sway, Hyprland, X11 sessions).
- Self-hosting Excalidraw's fonts inside the editor so it works offline.

## Reporting bugs

Open an issue with your OS, desktop environment, session type (Wayland or X11), the number of monitors and their scaling, what you did, what you expected, and what happened. Include the terminal output if you ran the app with `npm run tauri dev`.

## Commits and pull requests

- Write commit messages in the imperative: "Add monitor capture", not "Added".
- Describe what changed and why in the pull request, and link the issue if there is one.

## Releases

Packages are built by GitHub Actions ([`release.yml`](.github/workflows/release.yml)), and only after the CI matrix passes on Linux, macOS and Windows.

| Branch | What is published |
|---|---|
| `main` | A **pre-release** tagged `v<version>-rc.<run number>`, for example `v0.1.0-rc.7`. Use these to try changes. |
| `release` | The **real release**, tagged `v<version>`. |

To cut a release:

1. Bump the version to the same value in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `package.json`. The pipeline fails if they differ.
2. Merge to `main` and check that the latest pre-release installs and works.
3. Fast-forward `release` to `main`:

   ```bash
   git push origin main:release
   ```

The pipeline refuses to publish if the tag for that version already exists, so a release always needs a version bump. The installers are `.deb`, `.rpm` and AppImage (Linux), a universal `.dmg` (macOS), and `.msi` and `.exe` (Windows). They are not code-signed yet.

## License

By contributing you agree that your contribution is licensed under the [MIT License](LICENSE).
