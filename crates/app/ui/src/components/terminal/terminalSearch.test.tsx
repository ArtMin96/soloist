// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import { Terminal } from "@xterm/xterm";
import { NO_MATCHES, useTerminalSearch } from "@/components/terminal/terminalSearch";

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

// A hook attached to a real, open terminal holding three lines that each contain "err".
async function searching() {
  const term = new Terminal({ allowProposedApi: true, cols: 40, rows: 10 });
  const host = document.createElement("div");
  document.body.appendChild(host);
  const hook = renderHook(() => useTerminalSearch(() => false));
  let detach = () => {};
  await act(async () => {
    detach = hook.result.current.attach(term);
    term.open(host);
    await new Promise<void>((resolve) => term.write(OUTPUT, resolve));
  });
  return { hook, term, detach, matches: () => hook.result.current.search.matches };
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
    expect(matches()).toEqual({ index: -1, count: 0 });
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
