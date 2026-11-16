import { Component, Suspense, lazy, useEffect, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { SliceSelection } from "./Annotator";

// Only loaded when "annotate on the frozen screen" is on; preloaded as soon as a capture arms.
const loadAnnotator = () => import("./Annotator");
const Annotator = lazy(loadAnnotator);

/** Shows an annotator crash on screen (with a way out) instead of a blank page. */
class AnnotatorBoundary extends Component<{ children: ReactNode }, { error?: string }> {
  state: { error?: string } = {};
  static getDerivedStateFromError(e: unknown) {
    return { error: String(e) };
  }
  componentDidCatch(e: unknown) {
    console.error("annotator crashed", e);
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div style={{ position: "absolute", top: 16, left: 16, padding: 12, background: "#fff", color: "#900", borderRadius: 6, fontFamily: "sans-serif" }}>
        Annotating failed: {this.state.error}{" "}
        <button onClick={() => invoke("cancel_capture")}>Close</button>
      </div>
    );
  }
}

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
  /** Set once a region is selected with "annotate on the frozen screen" on. */
  const [annotating, setAnnotating] = useState<SliceSelection | null>(null);
  /** The selection in CSS pixels, for the picture shown underneath the annotator. */
  const [annotBox, setAnnotBox] = useState<{ left: number; top: number; width: number; height: number } | null>(null);
  const annotatingRef = useRef(false);
  const picked = useRef(false);
  const urlRef = useRef<string | undefined>(undefined);

  useEffect(() => {
    // The page background must never flash white while the window appears.
    document.documentElement.style.background = "#000";
    document.body.style.background = "#000";

    const reset = () => {
      picked.current = false;
      annotatingRef.current = false;
      setAnnotating(null);
      setAnnotBox(null);
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
        const m = await invoke<{ mode: string; annotate: boolean } | null>("overlay_mode");
        if (!m) return;
        reset();
        if (m.annotate) loadAnnotator().catch(() => {}); // warm the chunk while the user drags
        const bytes = await invoke<ArrayBuffer>("overlay_slice", { index });
        const url = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        urlRef.current = url;
        const img = new Image();
        img.src = url;
        await img.decode();
        setMode(m.mode);
        setSrc(url);
        // rAF does not run while the window is hidden, so give React a moment to commit the image.
        await new Promise((r) => setTimeout(r, 40));
        await invoke("overlay_ready", { index });
      } catch (e) {
        console.error("overlay arm failed", e);
      }
    };

    // While annotating, the annotator decides what Esc means (Excalidraw uses it too).
    const key = (e: KeyboardEvent) => e.key === "Escape" && !annotatingRef.current && invoke("cancel_capture");
    window.addEventListener("keydown", key);
    const unlisten = Promise.all([
      listen("overlay-arm", arm),
      listen("overlay-reset", reset),
      // After a selection with "annotate on the frozen screen": everyone but the chosen overlay resets.
      listen<number>("overlay-reset-others", (e) => e.payload !== index && reset()),
    ]);
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
        // Once annotating, pointer events belong to Excalidraw; capturing them here would break drawing and the buttons.
        if (mode !== "region" || annotatingRef.current) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        setDrag({ x0: e.clientX, y0: e.clientY, x1: e.clientX, y1: e.clientY });
      }}
      onPointerUp={(e) => {
        if (!drag) return;
        const w = e.clientX - drag.x0;
        const h = e.clientY - drag.y0;
        setDrag(null);
        if (Math.abs(w) < 4 || Math.abs(h) < 4) return;
        invoke<SliceSelection | null>("overlay_select", {
          index, x: drag.x0, y: drag.y0, w, h, ow: window.innerWidth, oh: window.innerHeight,
        }).then((sel) => {
          if (sel) {
            annotatingRef.current = true;
            setAnnotBox({ left: Math.min(drag.x0, drag.x0 + w), top: Math.min(drag.y0, drag.y0 + h), width: Math.abs(w), height: Math.abs(h) });
            setAnnotating(sel);
          }
        });
      }}
      style={{
        position: "fixed", inset: 0, background: "#000",
        cursor: mode === "region" ? "crosshair" : "default", userSelect: "none", overflow: "hidden",
      }}
    >
      {src && <img src={src} draggable={false} style={{ width: "100vw", height: "100vh", display: "block" }} />}
      {annotating && annotBox && (
        <div
          style={{
            position: "absolute", ...annotBox,
            boxShadow: "0 0 0 100vmax rgba(0,0,0,0.45)",
          }}
        />
      )}
      {src && annotating && (
        <AnnotatorBoundary>
          <Suspense fallback={null}>
            <Annotator src={src} sel={annotating} />
          </Suspense>
        </AnnotatorBoundary>
      )}
      {mode === "region" && !annotating && (
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
