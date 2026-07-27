// The single place the system clipboard is read and written for the terminal, so the backing
// implementation can be swapped without touching call sites. It goes through the OS rather than the
// webview's async Clipboard API: WebKitGTK gates `navigator.clipboard.readText()` behind a user
// gesture it does not credit a capture-phase key handler with, so a paste chord could not read at
// all. The plugin's commands run in the app process, where no such gate applies.
//
// Neither function rejects. A refused clipboard degrades — a write is dropped, a read yields no
// text — so the key handler and the selection listener that call these can never take an exception.
// A refusal is real rather than theoretical: the two commands are reachable only because
// `capabilities/default.json` grants `clipboard-manager:allow-read-text` and `allow-write-text`,
// and the runtime authority rejects the call outright if a grant is ever dropped.

import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";

export async function writeClipboard(text: string): Promise<void> {
  try {
    await writeText(text);
  } catch {
    // The write was refused; the clipboard keeps whatever it held.
  }
}

export async function readClipboard(): Promise<string> {
  try {
    return await readText();
  } catch {
    // A clipboard holding nothing, or holding something that is not text, rejects rather than
    // resolving empty — as does a refused read.
    return "";
  }
}
