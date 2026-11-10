import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Drag = { x0: number; y0: number; x1: number; y1: number } | null;

/**
 * One per monitor, created hidden at startup. When a capture starts Rust sends `overlay-arm`;
 * we fetch our frozen slice, wait until it is decoded, then call `overlay_ready` so the window
 * is only shown with its image already on screen. `overlay-reset` clears it again.
 *
 * Region mode: drag to select. Monitor mode: the first pointer event picks this monitor.
 */
export default function Overlay({ index }: { index: number }) {
  const [mode, setMode] = useState<string | null>(null);
  const [src, setSrc] = useState<string>();
  const [drag, setDrag] = useState<Drag>(null);
  const picked = useRef(false);
  const urlRef = useRef<string | undefined>(undefined);

  useEffect(() => {
    // The page background must never flash white while the window appears.
    document.documentElement.style.background = "#000";
    document.body.style.background = "#000";

    const reset = () => {
      picked.current = false;
      setDrag(null);
      setSrc(undefined);
      setMode(null);
      if (urlRef.current) {
        URL.revokeObjectURL(urlRef.current);
        urlRef.current = undefined;
      }
    };

    const arm = async () => {
      try {
        const m = await invoke<string | null>("overlay_mode");
        if (!m) return;
        reset();
        const bytes = await invoke<ArrayBuffer>("overlay_slice", { index });
        const url = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        urlRef.current = url;
        const img = new Image();
        img.src = url;
        await img.decode();
        setMode(m);
        setSrc(url);
        // rAF does not run while the window is hidden, so give React a moment to commit the image.
        await new Promise((r) => setTimeout(r, 40));
        await invoke("overlay_ready", { index });
      } catch (e) {
        console.error("overlay arm failed", e);
      }
    };

    const key = (e: KeyboardEvent) => e.key === "Escape" && invoke("cancel_capture");
    window.addEventListener("keydown", key);
    const unlisten = Promise.all([listen("overlay-arm", arm), listen("overlay-reset", reset)]);
    arm(); // covers a window that was created after the capture started

    return () => {
      window.removeEventListener("keydown", key);
      unlisten.then((fns) => fns.forEach((f) => f()));
    };
  }, [index]);

  const pick = () => {
    if (mode === "monitor" && !picked.current) {
      picked.current = true;
      invoke("overlay_pick", { index });
    }
  };

  const box = drag && {
    left: Math.min(drag.x0, drag.x1),
    top: Math.min(drag.y0, drag.y1),
    width: Math.abs(drag.x1 - drag.x0),
    height: Math.abs(drag.y1 - drag.y0),
  };

  return (
    <div
      onPointerEnter={pick}
      onPointerMove={(e) => {
        pick();
        if (drag) setDrag({ ...drag, x1: e.clientX, y1: e.clientY });
      }}
      onPointerDown={(e) => {
        if (mode !== "region") return;
        e.currentTarget.setPointerCapture(e.pointerId);
        setDrag({ x0: e.clientX, y0: e.clientY, x1: e.clientX, y1: e.clientY });
      }}
      onPointerUp={(e) => {
        if (!drag) return;
        const w = e.clientX - drag.x0;
        const h = e.clientY - drag.y0;
        setDrag(null);
        if (Math.abs(w) < 4 || Math.abs(h) < 4) return;
        invoke("overlay_select", {
          index, x: drag.x0, y: drag.y0, w, h, ow: window.innerWidth, oh: window.innerHeight,
        });
      }}
      style={{
        position: "fixed", inset: 0, background: "#000",
        cursor: mode === "region" ? "crosshair" : "default", userSelect: "none", overflow: "hidden",
      }}
    >
      {src && <img src={src} draggable={false} style={{ width: "100vw", height: "100vh", display: "block" }} />}
      {mode === "region" && (
        <>
          {!box && <div style={{ position: "absolute", inset: 0, background: "rgba(0,0,0,0.3)" }} />}
          {box && (
            <div
              style={{
                position: "absolute", ...box,
                outline: "1px solid #fff",
                boxShadow: "0 0 0 100vmax rgba(0,0,0,0.45)",
              }}
            />
          )}
        </>
      )}
    </div>
  );
}
