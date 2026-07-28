// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import { Terminal } from "@xterm/xterm";
import {
  NO_ACTIVE_MATCH,
  NO_MATCHES,
  useTerminalSearch,
} from "@/components/terminal/terminalSearch";
import { searchDecorationColors } from "@/lib/terminalPalette";

// Opening a terminal reads the OS colour-scheme preference, which jsdom does not implement. The
// search runs against a really-opened emulator here — the addon needs its selection and decoration
// services — so this one boundary is stubbed rather than the emulator being faked away.
beforeAll(() => {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener() {},
      removeEventListener() {},
      addListener() {},
      removeListener() {},
      dispatchEvent: () => false,
    }),
  });
});

const OUTPUT = "err one\r\nerr two\r\nerr three\r\n";

afterEach(cleanup);

// A hook attached to a real, open terminal holding `output`, which by default is three lines that
// each contain "err". The theme is a prop so a test can flip it the way the app does.
async function searching(output = OUTPUT) {
  const term = new Terminal({ allowProposedApi: true, cols: 40, rows: 10 });
  const host = document.createElement("div");
  document.body.appendChild(host);
  const hook = renderHook(({ dark }) => useTerminalSearch(dark), { initialProps: { dark: false } });
  let detach = () => {};
  await act(async () => {
    detach = hook.result.current.attach(term);
    term.open(host);
    await new Promise<void>((resolve) => term.write(output, resolve));
  });
  return { hook, term, host, detach, matches: () => hook.result.current.search.matches };
}

// The emulator attaches a decoration's element on a render frame, so the DOM the user would see
// lags the search that asked for it by one.
async function painted() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 100));
  });
}

// The border colour of every search decoration drawn on the pane, sorted so the set can be
// compared without depending on the order the emulator happens to attach them in. The addon writes
// the border as an inline outline, which makes it the one decoration colour the DOM can be asked
// for — the fills are the renderer's, and it draws to a canvas.
function decorationBorders(host: HTMLElement): string[] {
  return Array.from(host.querySelectorAll<HTMLElement>(".xterm-find-result-decoration"))
    .map((element) => element.style.outline)
    .sort();
}

// What `decorationBorders` should read once `matchCount` matches are painted for `dark` and one of
// them is the active match: the addon draws every match, then the active one again on top.
function expectedBorders(dark: boolean, matchCount: number): string[] {
  const colors = searchDecorationColors(dark);
  return [
    ...Array.from({ length: matchCount }, () => `1px solid ${colors.matchBorder}`),
    `1px solid ${colors.activeMatchBorder}`,
  ].sort();
}

describe("useTerminalSearch", () => {
  it("counts every match and tracks which one the view is standing on", async () => {
    const { hook, matches } = await searching();
    expect(matches()).toEqual(NO_MATCHES);

    await act(async () => hook.result.current.search.findNext("err"));
    expect(matches()).toEqual({ index: 0, count: 3 });

    await act(async () => hook.result.current.search.findNext("err"));
    expect(matches()).toEqual({ index: 1, count: 3 });

    await act(async () => hook.result.current.search.findNext("err"));
    expect(matches()).toEqual({ index: 2, count: 3 });
  });

  it("steps backwards through the same matches", async () => {
    const { hook, matches } = await searching();
    await act(async () => hook.result.current.search.findNext("err"));
    await act(async () => hook.result.current.search.findNext("err"));
    expect(matches()).toEqual({ index: 1, count: 3 });

    await act(async () => hook.result.current.search.findPrevious("err"));
    expect(matches()).toEqual({ index: 0, count: 3 });
  });

  it("reports no matches for a query that is not in the output", async () => {
    const { hook, matches } = await searching();
    await act(async () => hook.result.current.search.findNext("absent"));
    expect(matches()).toEqual({ index: NO_ACTIVE_MATCH, count: 0 });
  });

  it("redraws the matches already on screen in the new palette when the theme flips", async () => {
    const { hook, term, host, matches } = await searching();
    await act(async () => hook.result.current.search.findNext("err"));
    await act(async () => hook.result.current.search.findNext("err"));
    await painted();
    expect(decorationBorders(host)).toEqual(expectedBorders(false, 3));
    const standingOn = term.getSelectionPosition();

    await act(async () => hook.rerender({ dark: true }));
    await painted();

    expect(decorationBorders(host)).toEqual(expectedBorders(true, 3));
    // The repaint reissues the query, so it has to leave the user where they were: on the second
    // of three matches, with that match still selected.
    expect(matches()).toEqual({ index: 1, count: 3 });
    expect(term.getSelectionPosition()).toEqual(standingOn);
  });

  it("forgets the tally when the search is cleared, rather than leaving a stale count up", async () => {
    const { hook, matches } = await searching();
    await act(async () => hook.result.current.search.findNext("err"));
    expect(matches()).toEqual({ index: 0, count: 3 });

    await act(async () => hook.result.current.search.clear());
    expect(matches()).toEqual(NO_MATCHES);
  });

  it("counts a narrower query separately rather than carrying the previous one over", async () => {
    const { hook, matches } = await searching();
    await act(async () => hook.result.current.search.findNext("err"));
    expect(matches()).toEqual({ index: 0, count: 3 });

    await act(async () => hook.result.current.search.findNext("three"));
    expect(matches()).toEqual({ index: 0, count: 1 });
  });

  it("stops reporting once the pane detaches, leaving nothing pointing at a dead terminal", async () => {
    const { hook, detach, matches } = await searching();
    await act(async () => hook.result.current.search.findNext("err"));
    expect(matches()).toEqual({ index: 0, count: 3 });

    await act(async () => detach());
    expect(matches()).toEqual(NO_MATCHES);

    // The addon is released with the pane, so a late search is inert rather than reviving a count.
    await act(async () => hook.result.current.search.findNext("err"));
    expect(matches()).toEqual(NO_MATCHES);
  });
});
