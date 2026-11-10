import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Excalidraw, convertToExcalidrawElements, exportToBlob } from "@excalidraw/excalidraw";
import "@excalidraw/excalidraw/index.css";

const FILE_ID = "screenshot";

function toDataURL(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(r.result as string);
    r.onerror = reject;
    r.readAsDataURL(blob);
  });
}

/** Excalidraw canvas with the capture as a locked background image, plus Copy / Save. */
export default function Editor() {
  // Excalidraw's imperative API type is not worth importing for this small surface.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const apiRef = useRef<any>(null);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [initial, setInitial] = useState<any>(null);
  const [msg, setMsg] = useState("");

  useEffect(() => {
    (async () => {
      const buf = await invoke<ArrayBuffer>("editor_image");
      const blob = new Blob([buf], { type: "image/png" });
      const { width, height } = await createImageBitmap(blob);
      const dataURL = await toDataURL(blob);
      const elements = convertToExcalidrawElements([
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        { type: "image", fileId: FILE_ID, x: 0, y: 0, width, height, status: "saved", locked: true } as any,
      ]);
      setInitial({
        elements,
        files: { [FILE_ID]: { id: FILE_ID, mimeType: "image/png", dataURL, created: Date.now() } },
        appState: { viewBackgroundColor: "#ffffff" },
        scrollToContent: true,
      });
    })().catch((e) => setMsg(`load failed: ${e}`));
  }, []);

  /** Annotated scene as PNG at native resolution (1 scene unit = 1 screenshot pixel). */
  const exportPng = useCallback(async () => {
    const api = apiRef.current;
    const blob = await exportToBlob({
      elements: api.getSceneElements(),
      appState: { ...api.getAppState(), exportBackground: true, exportScale: 1 },
      files: api.getFiles(),
      mimeType: "image/png",
      exportPadding: 0,
    });
    return new Uint8Array(await blob.arrayBuffer());
  }, []);

  const run = useCallback(
    async (cmd: "copy_png" | "save_png") => {
      try {
        const r = await invoke<string | null>(cmd, await exportPng());
        if (r) setMsg(`Saved ${r}`);
      } catch (e) {
        setMsg(`${cmd} failed: ${e}`);
      }
    },
    [exportPng],
  );

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
  }, [run]);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100vh", fontFamily: "sans-serif" }}>
      <div style={{ padding: 8, display: "flex", gap: 8, alignItems: "center", borderBottom: "1px solid #8884" }}>
        <button onClick={() => run("copy_png")} title="Ctrl+Enter, or Ctrl+C with nothing selected">
          Copy
        </button>
        <button onClick={() => run("save_png")} title="Ctrl+S">
          Save
        </button>
        <span style={{ fontSize: 12 }}>{msg}</span>
      </div>
      <div style={{ flex: 1, minHeight: 0 }}>
        {initial && (
          <Excalidraw
            initialData={initial}
            excalidrawAPI={(api) => (apiRef.current = api)}
            UIOptions={{
              canvasActions: { export: false, loadScene: false, saveToActiveFile: false, saveAsImage: false },
            }}
          />
        )}
      </div>
    </div>
  );
}
