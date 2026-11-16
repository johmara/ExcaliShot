import { useEffect, type MutableRefObject } from "react";

export function toDataURL(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(r.result as string);
    r.onerror = reject;
    r.readAsDataURL(blob);
  });
}

/**
 * Ctrl+S saves, Ctrl+Enter copies, and so does Ctrl+C when nothing is selected (and you are
 * not typing). Shared by the editor window and the on-screen annotator.
 */
export function useCopySaveKeys(
  // Excalidraw's imperative API type is not worth importing for this small surface.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  apiRef: MutableRefObject<any>,
  run: (cmd: "copy_png" | "save_png") => void,
) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      const api = apiRef.current;
      if (!mod || !api) return;
      const t = e.target as HTMLElement;
      const typing = t.tagName === "TEXTAREA" || t.tagName === "INPUT" || t.isContentEditable;
      if (e.key === "s") {
        e.preventDefault();
        run("save_png");
      } else if (e.key === "Enter") {
        e.preventDefault();
        run("copy_png");
      } else if (e.key === "c" && !typing && Object.keys(api.getAppState().selectedElementIds).length === 0) {
        // Ctrl+C with nothing selected means "copy the screenshot".
        e.preventDefault();
        run("copy_png");
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [apiRef, run]);
}
