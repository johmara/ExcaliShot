import Overlay from "./overlay/Overlay";
import Editor from "./editor/Editor";
import Settings from "./settings/Settings";

// One webview bundle, routed by query string: ?overlay=<i>&mode=..., ?editor=1, otherwise settings.
function App() {
  const q = new URLSearchParams(window.location.search);
  const overlay = q.get("overlay");
  if (overlay !== null) return <Overlay index={Number(overlay)} />;
  if (q.get("editor")) return <Editor />;
  return <Settings />;
}

export default App;
