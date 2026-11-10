import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Drag = { x0: number; y0: number; x1: number; y1: number } | null;

/** Frozen picture of one monitor. Region mode: drag to select. Monitor mode: first pointer event picks it. */
export default function Overlay({ index, mode }: { index: number; mode: string }) {
  const [src, setSrc] = useState<string>();
  const [drag, setDrag] = useState<Drag>(null);
  const done = useRef(false);

  useEffect(() => {
    let url: string | undefined;
    invoke<ArrayBuffer>("overlay_slice", { index }).then((b) => {
      url = URL.createObjectURL(new Blob([b], { type: "image/png" }));
      setSrc(url);
    });
    const key = (e: KeyboardEvent) => e.key === "Escape" && invoke("cancel_capture");
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("keydown", key);
      if (url) URL.revokeObjectURL(url);
    };
  }, [index]);

  const pick = () => {
    if (mode === "monitor" && !done.current) {
      done.current = true;
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
      style={{ position: "fixed", inset: 0, cursor: mode === "region" ? "crosshair" : "default", userSelect: "none", overflow: "hidden" }}
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
