import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./settings.css";

type ShortcutStatus = {
  backend: string;
  error: string | null;
  shortcuts: { id: string; description: string; trigger: string }[];
};

/** Hand-drawn underline, in the spirit of Excalidraw's marker strokes. */
function Squiggle() {
  return (
    <svg className="squiggle" width="170" height="10" viewBox="0 0 170 10" fill="none" aria-hidden="true">
      <path
        d="M2 6 C 20 1, 34 9, 54 5 S 90 2, 108 6 S 146 9, 168 3"
        stroke="currentColor"
        strokeWidth="2.5"
        strokeLinecap="round"
      />
    </svg>
  );
}

function Keys({ trigger }: { trigger: string }) {
  if (!trigger) return <span className="unassigned">not assigned</span>;
  return (
    <span>
      {trigger.split("+").map((k, i) => (
        <kbd key={i}>{k}</kbd>
      ))}
    </span>
  );
}

export default function Settings() {
  const [status, setStatus] = useState<ShortcutStatus>();
  const [msg, setMsg] = useState("");
  const [autostart, setAutostart] = useState(false);

  useEffect(() => {
    invoke<boolean>("autostart_get").then(setAutostart, () => {});
  }, []);

  // Shortcut binding finishes asynchronously after startup (and may wait on a GNOME dialog), so poll briefly.
  useEffect(() => {
    let n = 0;
    const t = setInterval(async () => {
      const s = await invoke<ShortcutStatus>("shortcut_status");
      setStatus(s);
      if ((s.backend && (s.shortcuts.length || s.error)) || ++n > 60) clearInterval(t);
    }, 1000);
    return () => clearInterval(t);
  }, []);

  // Delay so you can move the mouse to the monitor you want, and this window is out of the shot.
  const capture = (mode: string) => setTimeout(() => invoke("start_capture", { mode }), 1500);

  const changeShortcuts = () =>
    invoke("configure_shortcuts").then(
      () => setMsg(""),
      (e) =>
        setMsg(
          String(e).includes("requires version")
            ? "This desktop cannot open the shortcut dialog from an app. On GNOME: Settings > Apps > ExcaliShot > Shortcuts."
            : `Could not open shortcut settings: ${e}`,
        ),
    );

  return (
    <main className="settings">
      <h1>ExcaliShot</h1>
      <Squiggle />
      <p className="tagline">Capture, sketch on it, share it.</p>

      <section className="island">
        <h2>Capture now</h2>
        <div className="btn-row">
          <button className="btn primary" onClick={() => capture("region")}>
            Region
          </button>
          <button className="btn primary" onClick={() => capture("monitor")}>
            Current monitor
          </button>
          <button className="btn primary" onClick={() => capture("full")}>
            All monitors
          </button>
        </div>
        <p className="hint">Starts after 1.5 s so you can move the mouse and get this window out of the way.</p>
      </section>

      <section className="island">
        <h2>Global shortcuts</h2>
        {!status?.backend && <p className="hint">Binding shortcuts... (GNOME may show a confirmation dialog)</p>}
        {status?.error && <p className="error">Unavailable: {status.error}</p>}
        <ul className="shortcut-list">
          {status?.shortcuts.map((s) => (
            <li key={s.id}>
              <span>{s.description}</span>
              <Keys trigger={s.trigger} />
            </li>
          ))}
        </ul>
        <button className="btn" onClick={changeShortcuts}>
          Change shortcuts...
        </button>
        {msg && <p className="hint">{msg}</p>}
      </section>

      <section className="island">
        <h2>Startup</h2>
        <label className="toggle">
          <input
            type="checkbox"
            checked={autostart}
            onChange={(e) => {
              const enabled = e.currentTarget.checked;
              invoke("autostart_set", { enabled }).then(
                () => setAutostart(enabled),
                (err) => setMsg(`Could not change autostart: ${err}`),
              );
            }}
          />
          Start ExcaliShot when I log in
        </label>
      </section>
    </main>
  );
}
