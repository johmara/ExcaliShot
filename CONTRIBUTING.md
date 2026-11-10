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

One GitHub Actions workflow ([`release.yml`](.github/workflows/release.yml)) does both jobs. On pull requests it builds and tests on Linux, macOS and Windows. On pushes to `main` and on release tags it does the same and then packages and publishes, per platform, only if that platform's unit tests passed.

| Trigger | What is published |
|---|---|
| Push to `main` | A **pre-release** tagged `v<version>-rc.<run number>`, for example `v0.1.1-rc.7`. Use these to try changes. |
| Push of a tag `vX.Y.Z` | The **real release** `vX.Y.Z`. |

To cut a release:

1. Check that the latest pre-release from `main` installs and works.
2. Tag the commit on `main` with the version it carries (the `version` in `src-tauri/tauri.conf.json`) and push the tag:

   ```bash
   git tag v0.1.1 && git push origin v0.1.1
   ```

   You can also use **Create release** on GitHub with a new tag `v0.1.1` that targets `main`. The pipeline then builds the packages and attaches them to that release.

The pipeline refuses to publish if the tag does not match the app version, is not on `main`, or if the three version files (`tauri.conf.json`, `Cargo.toml`, `package.json`) disagree.

**The version bumps itself.** After a release is published, the pipeline commits "Bump version to ..." to `main` with the next patch version, so the following pre-releases belong to the upcoming release. For a minor or major release, change the version in the three files by hand (and update `package-lock.json` and `src-tauri/Cargo.lock` with `npm install` and `cargo check`) before tagging.

**Pre-releases are cleaned up.** When a real release is published, the pipeline deletes every pre-release (and its `-rc` tag), so the Releases page only shows real releases plus the pre-releases built since. This runs only after all platform packages were published successfully.

### Update signing

Installed apps update themselves from the release assets (`latest.json` and the signed packages). The CI build signs them with a minisign key: the private key and its password are the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, and the matching public key is `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`. Keep a backup of the private key and password outside GitHub: **if they are lost or replaced, installed copies can no longer verify updates, and everyone has to reinstall by hand.** Never commit the key or paste it into issues or chats. To build installers locally, `npm run tauri build` needs the key available as `TAURI_SIGNING_PRIVATE_KEY` (and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) in the environment, or you can pass `--config '{"bundle":{"createUpdaterArtifacts":false}}'`.

The installers are `.deb`, `.rpm` and AppImage (Linux), a universal `.dmg` (macOS), and `.msi` and `.exe` (Windows). They are not code-signed yet.

## License

By contributing you agree that your contribution is licensed under the [MIT License](LICENSE).
