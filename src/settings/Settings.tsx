import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import "./settings.css";

const RELEASES_URL = "https://github.com/johmara/ExcaliShot/releases/latest";

type UpdateStatus =
  | { state: "idle" | "checking" | "upToDate" | "installing" }
  | { state: "available"; version: string; notes: string | null; canInstall: boolean }
  | { state: "error"; message: string };

type ShortcutStatus = {
  backend: string;
  /** False when the desktop owns the shortcuts (Linux portal). */
  editable: boolean;
  error: string | null;
  shortcuts: { id: string; description: string; trigger: string; problem: string | null }[];
};

/** Build an accelerator like "Ctrl+Shift+1" from a key press. Returns null until a real key with a modifier is pressed. */
function accelFromEvent(e: KeyboardEvent): string | null {
  if (/^(Control|Shift|Alt|Meta|OS)(Left|Right)?$/.test(e.code)) return null;
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");
  if (mods.length === 0) return null; // a bare key would hijack normal typing everywhere
  const key = e.code.startsWith("Key")
    ? e.code.slice(3)
    : e.code.startsWith("Digit")
      ? e.code.slice(5)
      : e.code;
  return [...mods, key].join("+");
}

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
  const [update, setUpdate] = useState<UpdateStatus>({ state: "idle" });
  const [version, setVersion] = useState("");

  // The backend checks for updates at startup and from the tray; follow whatever it reports.
  useEffect(() => {
    getVersion().then(setVersion, () => {});
    invoke<UpdateStatus>("update_status").then(setUpdate, () => {});
    const unlisten = listen<UpdateStatus>("update-status", (e) => setUpdate(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);
  /** Id of the shortcut currently waiting for a new key combination. */
  const [recording, setRecording] = useState<string | null>(null);

  useEffect(() => {
    invoke<boolean>("autostart_get").then(setAutostart, () => {});
  }, []);

  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setRecording(null);
        return;
      }
      const accelerator = accelFromEvent(e);
      if (!accelerator) return;
      setRecording(null);
      invoke<ShortcutStatus>("set_shortcut", { id: recording, accelerator }).then(
        (s) => {
          setStatus(s);
          setMsg("");
        },
        (err) => setMsg(String(err)),
      );
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording]);

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
              <span>
                {s.description}
                {s.problem && <span className="error"> ({s.problem})</span>}
              </span>
              <span className="row-end">
                {recording === s.id ? (
                  <span className="unassigned">Press the new shortcut... (Esc cancels)</span>
                ) : (
                  <Keys trigger={s.trigger} />
                )}
                {status?.editable && recording !== s.id && (
                  <button
                    className="btn small"
                    onClick={() => {
                      setMsg("");
                      setRecording(s.id);
                    }}
                  >
                    Change
                  </button>
                )}
              </span>
            </li>
          ))}
        </ul>
        {status && !status.editable && (
          <button className="btn" onClick={changeShortcuts}>
            Change shortcuts...
          </button>
        )}
        {msg && <p className="hint">{msg}</p>}
      </section>

      <section className="island">
        <h2>Updates</h2>
        <p className="hint" style={{ marginTop: 0 }}>
          {version && `Version ${version}. `}
          {update.state === "checking" && "Checking for updates..."}
          {update.state === "upToDate" && "You are up to date."}
          {update.state === "installing" && "Downloading and installing the update..."}
          {update.state === "error" && `Could not check for updates: ${update.message}`}
          {update.state === "available" && `Version ${update.version} is available.`}
        </p>
        {update.state === "available" && update.notes && <p className="hint">{update.notes}</p>}
        <div className="btn-row">
          {update.state === "available" && update.canInstall && (
            <button className="btn primary" onClick={() => invoke("install_update").catch(() => {})}>
              Install {update.version} and restart
            </button>
          )}
          {update.state === "available" && !update.canInstall && (
            <button className="btn primary" onClick={() => openUrl(RELEASES_URL)}>
              Download {update.version} from GitHub
            </button>
          )}
          <button
            className="btn"
            disabled={update.state === "checking" || update.state === "installing"}
            onClick={() => invoke("check_for_update").catch(() => {})}
          >
            Check for updates
          </button>
        </div>
        {update.state === "available" && !update.canInstall && (
          <p className="hint">This installation cannot update itself; install the new package over it.</p>
        )}
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
