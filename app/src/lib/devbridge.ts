// Development only: when the UI runs in a plain browser (no Tauri), forward all commands
// to the craze-devbridge HTTP server. Used for automated UI tests.
import { mockIPC } from "@tauri-apps/api/mocks";

export function installDevBridge(url = "http://127.0.0.1:1421") {
  mockIPC(async (cmd, args) => {
    if (cmd.startsWith("plugin:")) {
      // Dialogs etc.: tests set the path through window.__crazeDialogPath.
      if (cmd === "plugin:dialog|save" || cmd === "plugin:dialog|open") {
        return (window as unknown as { __crazeDialogPath?: string }).__crazeDialogPath ?? null;
      }
      if (cmd === "plugin:event|listen") return 0;
      return null;
    }
    const body = JSON.stringify(args ?? {}, (_k, v) => (v && typeof v === "object" && "onmessage" in v ? null : v));
    const r = await fetch(`${url}/invoke/${cmd}`, { method: "POST", body });
    const ct = r.headers.get("Content-Type") ?? "";
    const data = ct.includes("octet-stream") ? await r.arrayBuffer() : await r.json();
    if (!r.ok) throw data;
    return data;
  });
}
