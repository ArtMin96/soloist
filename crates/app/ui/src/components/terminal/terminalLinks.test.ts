// Drives the *real* `@xterm/addon-web-links` matcher — no addon mock here — because the claim under
// test is which text the plain-text route is willing to turn into a link at all. The emulator is
// faked down to the two buffer calls the addon's link computer makes, which is enough to run its
// regex over a line of output. Nothing is activated, so no URL ever reaches the opener.

import { describe, expect, it } from "vitest";
import { webLinksAddon } from "@/components/terminal/terminalLinks";
import type { ILink, Terminal } from "@xterm/xterm";

interface LinkProvider {
  provideLinks(y: number, callback: (links: ILink[] | undefined) => void): void;
}

// One cell per character, every cell one column wide — the addon walks these to map a match's
// string offsets back to buffer coordinates.
function bufferOf(text: string) {
  const cell = {
    ch: "",
    getChars() {
      return this.ch;
    },
    getWidth() {
      return 1;
    },
  };
  const line = {
    isWrapped: false,
    length: text.length,
    translateToString: () => text,
    getCell(index: number, target: typeof cell) {
      target.ch = text[index] ?? "";
    },
  };
  // Only row 0 holds output; the addon reads past the end while looking for wrapped continuations.
  return {
    active: { getLine: (y: number) => (y === 0 ? line : undefined), getNullCell: () => cell },
  };
}

function linksOn(line: string): ILink[] {
  let registered: LinkProvider | undefined;
  const terminal = {
    buffer: bufferOf(line),
    registerLinkProvider: (provider: LinkProvider) => {
      registered = provider;
      return { dispose() {} };
    },
  };

  webLinksAddon(() => {}).activate(terminal as unknown as Terminal);
  if (!registered) throw new Error("the addon registered no link provider");

  let found: ILink[] = [];
  // `computeLink` reads row `y - 1`, so row 0 is asked for as 1.
  registered.provideLinks(1, (links) => {
    found = links ?? [];
  });
  return found;
}

describe("the plain-text link route", () => {
  it("linkifies an http and an https URL", () => {
    expect(
      linksOn("see http://a.example/x and https://b.example/y done").map((l) => l.text),
    ).toEqual(["http://a.example/x", "https://b.example/y"]);
  });

  // The scheme rule the pane depends on is upstream: this route never offers the app anything but
  // http(s), so `openExternal` is never asked to judge a `file:` path the user did not type. Widen
  // the addon's regex and these become links, which is the change this guards against.
  it("linkifies nothing when the output carries a local path or a script URI", () => {
    expect(linksOn("open file:///etc/passwd or javascript:alert(1) or data:text/html,x")).toEqual(
      [],
    );
  });

  it("linkifies neither a bare host nor a mail or telephone URI", () => {
    expect(linksOn("visit example.com or mailto:a@b.example or tel:+15550100")).toEqual([]);
  });
});
