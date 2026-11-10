# ExcaliShot logo

Concept: four hand-drawn capture brackets (the screenshot region) with a
hand-drawn Excalidraw-style arrow inside (the annotation). One bold silhouette,
wobbly strokes with round caps, no fine detail.

## Files

| File | Purpose |
| --- | --- |
| `logo.svg` | Master, colour. Opaque indigo tile, so it works on light and dark backgrounds. |
| `logo-mono.svg` | Single colour (black), transparent, no tile, heavier strokes. For tray / symbolic icons; recolour via `stroke`. |
| `logo-1024.png` | 1024x1024 export of `logo.svg` (source for `npx tauri icon`). |
| `concept-b-selection-box.svg` | Rejected: wobbly box with corner handles. Reads as a generic "select" tool. |
| `concept-c-crosshair.svg` | Rejected: crosshair circle. Reads as "target / aim", loses the hand-drawn arrow. |

## Palette

| Role | Hex |
| --- | --- |
| Tile (indigo, Excalidraw-adjacent) | `#5B57D1` |
| Brackets | `#FFFFFF` |
| Arrow (amber) | `#FFC53D` |
| Mono | `#000000` (or any single colour) |

## Usage notes

- Keep the tile corners transparent (rx 224 of 1024); do not add a drop shadow.
- Minimum size for the colour logo is 32 px. Below that (tray, 16-22 px) use `logo-mono.svg`.
- On macOS, use the mono file as a template image (black + alpha). On GNOME / Windows trays, recolour to white or black to match the panel.
- Clear space: at least 1/8 of the width around the tile.

## Regenerate PNG and icon set

```bash
rsvg-convert -w 1024 -h 1024 design/logo/logo.svg -o design/logo/logo-1024.png
npx tauri icon design/logo/logo-1024.png
```
