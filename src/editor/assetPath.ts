// Excalidraw loads its fonts from a CDN unless told where they are hosted. We ship them in
// public/fonts, so point it at our own origin. This module must be imported BEFORE
// "@excalidraw/excalidraw" (ES modules evaluate in import order).
declare global {
  interface Window {
    EXCALIDRAW_ASSET_PATH?: string | string[];
  }
}

window.EXCALIDRAW_ASSET_PATH = "/";

export {};
