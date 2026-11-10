## What and why

<!-- What changed, and why. Link the issue if there is one: "Closes #123". -->

## How it was tested

<!-- For anything that changes behaviour, say how you tested it by hand.
For capture, overlay or shortcut changes: OS, desktop, session type (Wayland or X11),
number of monitors and the scaling you tried, e.g. "Ubuntu 26.04, GNOME, Wayland, 2 monitors, 150%". -->

## Checklist

- [ ] `cd src-tauri && cargo test && cargo check` passes
- [ ] `npm run build` passes
- [ ] `cargo fmt` and `cargo clippy` run on the code I touched
- [ ] One topic per pull request
- [ ] No capture data is sent anywhere or written to disk except through the Save action
