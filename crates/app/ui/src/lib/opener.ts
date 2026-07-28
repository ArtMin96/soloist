// The single place the app hands a URL to the desktop. Every route that can open a link — a plain
// URL the terminal linkified, an OSC 8 hyperlink a supervised program emitted — goes through here,
// so the scheme check exists once instead of once per call site.
//
// The webview may never navigate to a remote origin itself: its CSP confines it to the app's own
// origin, so the system browser is the only place a link can legitimately land.

import { openUrl } from "@tauri-apps/plugin-opener";

// The only two schemes a browser is the right handler for. `file:` would hand a local path to the
// desktop; `javascript:` and `data:` are script chosen by whatever wrote the line. A URL printed by
// a supervised process is untrusted input, and none of those belong in it. The capability grants the
// same pair in the core process, so this guard is the convenience, not the security boundary.
const OPENABLE_PROTOCOLS = ["http:", "https:"];

/**
 * Open a URL in the desktop's default browser, if it is one we are willing to open. Anything
 * unparseable or outside {@link OPENABLE_PROTOCOLS} is dropped.
 *
 * Never rejects: a hostile or malformed link in a process's output is inert rather than an error
 * the terminal has to handle.
 */
export async function openExternal(url: string): Promise<void> {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return;
  }
  if (!OPENABLE_PROTOCOLS.includes(parsed.protocol)) return;
  try {
    // The parsed form, so the string that reaches the opener is the one whose scheme was checked.
    await openUrl(parsed.href);
  } catch {
    // No handler on the desktop, or the capability refused it; the link just does not open.
  }
}
