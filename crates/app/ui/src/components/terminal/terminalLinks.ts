// The two routes a URL can take out of a terminal pane: a plain-text URL the web-links addon finds
// by regex, and an OSC 8 hyperlink the emulator parses natively. Both end at `openExternal`, and
// both report what is under the pointer so the pane can show where the link actually goes. Kept
// beside the terminal hook rather than inside it so the emulator's stream lifecycle and its link
// behavior stay separately readable.
//
// Neither route may be left to its default. An activated link with no handler runs a blocking
// `confirm()` and then `window.open()`, and the addon's default handler is that same `window.open` —
// under the app's CSP neither reaches a remote origin.

import { WebLinksAddon } from "@xterm/addon-web-links";
import type { ILinkHandler } from "@xterm/xterm";
import { openExternal } from "@/lib/opener";

/** Reports the URI under the pointer, and null once it leaves. */
export type LinkTargetSink = (uri: string | null) => void;

// Both routes activate and report identically; only the shape they are handed to differs — the
// emulator takes one object, the addon takes the handler positionally and the rest as options.
const openLink = (_event: MouseEvent, uri: string) => void openExternal(uri);

function reportTarget(onTarget: LinkTargetSink) {
  return {
    hover: (_event: MouseEvent, uri: string) => onTarget(uri),
    leave: () => onTarget(null),
  };
}

/**
 * Handles OSC 8 hyperlinks, which the emulator parses itself.
 *
 * The URI passed here is the destination the emitting program set, not the text it chose to
 * display — OSC 8 allows the two to differ, which is exactly the shape a phishing link takes. The
 * hover readout is fed from this value for that reason, and never from what is on screen.
 *
 * `allowNonHttpProtocols` is deliberately left unset: while it is falsy the emulator drops every
 * OSC 8 link whose URI is not http(s) before it ever becomes clickable.
 */
export function oscLinkHandler(onTarget: LinkTargetSink): ILinkHandler {
  return { activate: openLink, ...reportTarget(onTarget) };
}

/** Linkifies plain-text URLs in the output. Its regex matches http(s) only, so nothing else is
 * ever turned into a link on this route. */
export function webLinksAddon(onTarget: LinkTargetSink): WebLinksAddon {
  return new WebLinksAddon(openLink, reportTarget(onTarget));
}
