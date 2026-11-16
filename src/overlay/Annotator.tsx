import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "../editor/assetPath"; // must stay above the Excalidraw import
import { Excalidraw, convertToExcalidrawElements, exportToCanvas, getCommonBounds } from "@excalidraw/excalidraw";
import "@excalidraw/excalidraw/index.css";
import { toDataURL, useCopySaveKeys } from "../editor/shared";

const FILE_ID = "screenshot";

/** A selection in the pixels of the monitor's frozen slice (from the `overlay_select` command). */
export type SliceSelection = { x: number; y: number; w: number; h: number; slice_w: number; slice_h: number };

/** The frozen slice with everything outside the selection dimmed. */
async function dimmedBackground(src: string, sel: SliceSelection): Promise<string> {
  const img = new Image();
  await new Promise<void>((resolve, reject) => {
    img.onload = () => resolve();
    img.onerror = () => reject(new Error("could not load the frozen screen"));
    img.src = src;
  });
  const canvas = document.createElement("canvas");
  canvas.width = sel.slice_w;
  canvas.height = sel.slice_h;
  const ctx = canvas.getContext("2d")!;
  ctx.drawImage(img, 0, 0, sel.slice_w, sel.slice_h);
  ctx.fillStyle = "rgba(0,0,0,0.45)";
  ctx.beginPath();
  ctx.rect(0, 0, sel.slice_w, sel.slice_h);
  ctx.rect(sel.x, sel.y, sel.w, sel.h);
  ctx.fill("evenodd");
  const blob = await new Promise<Blob>((resolve, reject) =>
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("canvas export failed"))), "image/png"),
  );
  return toDataURL(blob);
}

/**
 * "Annotate on the frozen screen": Excalidraw over the frozen monitor, with everything outside
 * the selection dimmed. Copy and Save export only the selection. Loaded lazily by the overlay.
 */
export default function Annotator({ src, sel }: { src: string; sel: SliceSelection }) {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const apiRef = useRef<any>(null);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [initial, setInitial] = useState<any>(null);
  const [msg, setMsg] = useState("");
  /** CSS pixels per slice pixel: the slice is shown fitted to the window. */
  const scale = window.innerWidth / sel.slice_w;

  useEffect(() => {
    dimmedBackground(src, sel)
      .then((dataURL) => {
        const elements = convertToExcalidrawElements([
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          { type: "image", fileId: FILE_ID, x: 0, y: 0, width: sel.slice_w, height: sel.slice_h, status: "saved", locked: true } as any,
        ]);
        setInitial({
          elements,
          files: { [FILE_ID]: { id: FILE_ID, mimeType: "image/png", dataURL, created: Date.now() } },
          appState: { viewBackgroundColor: "transparent", zoom: { value: scale }, scrollX: 0, scrollY: 0 },
        });
      })
      .catch((e) => setMsg(`load failed: ${e}`));
  }, [src, sel, scale]);

  /** The annotated selection as PNG at native resolution. */
  const exportPng = useCallback(async () => {
    const api = apiRef.current;
    const elements = api.getSceneElements();
    const [minX, minY] = getCommonBounds(elements);
    const full: HTMLCanvasElement = await exportToCanvas({
      elements,
      appState: { ...api.getAppState(), exportBackground: true, exportScale: 1 },
      files: api.getFiles(),
      exportPadding: 0,
    });
    // The export starts at the top-left of all elements, so shift by that to find the selection.
    const out = document.createElement("canvas");
    out.width = sel.w;
    out.height = sel.h;
    out.getContext("2d")!.drawImage(full, Math.round(sel.x - minX), Math.round(sel.y - minY), sel.w, sel.h, 0, 0, sel.w, sel.h);
    const blob = await new Promise<Blob>((resolve, reject) =>
      out.toBlob((b) => (b ? resolve(b) : reject(new Error("canvas export failed"))), "image/png"),
    );
    return new Uint8Array(await blob.arrayBuffer());
  }, [sel]);

  const run = useCallback(
    async (cmd: "copy_png" | "save_png") => {
      try {
        await invoke(cmd, await exportPng());
        await invoke("cancel_capture"); // done: clear the capture and hide the overlay
      } catch (e) {
        setMsg(`${cmd} failed: ${e}`);
      }
    },
    [exportPng],
  );

  useCopySaveKeys(apiRef, run);

  // Esc cancels the capture, like in the region selector. It is left to Excalidraw only while
  // something of Excalidraw's own is in progress: typing text, drawing a shape, or an open
  // dialog, menu or popup (Esc closes those first).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || !apiRef.current) return;
      const t = e.target as HTMLElement;
      const typing = t.tagName === "TEXTAREA" || t.tagName === "INPUT" || t.isContentEditable;
      const s = apiRef.current.getAppState();
      const busy =
        typing ||
        s.editingTextElement ||
        s.newElement ||
        s.multiElement ||
        s.selectedLinearElement?.isEditing ||
        s.openDialog ||
        s.openMenu ||
        s.openPopup;
      if (!busy) invoke("cancel_capture");
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  // Copy / Save buttons just under the selection's bottom-right corner, kept inside the window.
  const buttons = useMemo(() => {
    const right = (sel.x + sel.w) * scale;
    const below = (sel.y + sel.h) * scale + 8;
    const top = below + 40 > window.innerHeight ? Math.max(8, (sel.y + sel.h) * scale - 48) : below;
    return { right: Math.max(8, window.innerWidth - right), top };
  }, [sel, scale]);

  return (
    <div style={{ position: "fixed", inset: 0, fontFamily: "sans-serif" }}>
      {initial && (
        <Excalidraw
          initialData={initial}
          autoFocus // otherwise the keyboard shortcuts only work after clicking a tool
          excalidrawAPI={(api) => {
            apiRef.current = api;
          }}
          UIOptions={{
            canvasActions: { export: false, loadScene: false, saveToActiveFile: false, saveAsImage: false },
          }}
        />
      )}
      <div
        style={{
          position: "absolute", right: buttons.right, top: buttons.top, zIndex: 10,
          display: "flex", gap: 8, alignItems: "center", background: "#fffd", padding: 6, borderRadius: 6,
        }}
      >
        <button onClick={() => run("copy_png")} title="Ctrl+Enter, or Ctrl+C with nothing selected">
          Copy
        </button>
        <button onClick={() => run("save_png")} title="Ctrl+S">
          Save
        </button>
        <button onClick={() => invoke("cancel_capture")} title="Esc">
          Cancel
        </button>
        {msg && <span style={{ fontSize: 12 }}>{msg}</span>}
      </div>
    </div>
  );
}
