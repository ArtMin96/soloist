// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import { DETAIL_BACK_ATTRIBUTE } from "@/components/common/DetailPane";
import { PANEL_ATTRIBUTE } from "@/components/common/SlidingPanels";
import {
  createNavigationLedger,
  useMasterDetail,
  type MasterDetailOptions,
} from "@/store/useMasterDetail";

const PROJECT = 1;

/** How the stand-in board addresses one of its rows — the hook is told this through `rowTrigger`. */
const ROW_ATTRIBUTE = "data-row";

afterEach(() => {
  cleanup();
  document.body.innerHTML = "";
});

// The two panels the hook reaches into, standing in for a rendered board: it finds them by the
// handles the components emit, so the fixture wears exactly those and nothing else.
function panels(keys: number[]) {
  const list = document.createElement("div");
  list.setAttribute(PANEL_ATTRIBUTE, "list");
  for (const key of keys) {
    const row = document.createElement("button");
    row.setAttribute(ROW_ATTRIBUTE, String(key));
    list.append(row);
  }

  const detail = document.createElement("div");
  detail.setAttribute(PANEL_ATTRIBUTE, "detail");
  const back = document.createElement("button");
  back.setAttribute(DETAIL_BACK_ATTRIBUTE, "");
  detail.append(back);

  document.body.append(list, detail);

  return {
    back,
    row: (key: number) =>
      list.querySelector<HTMLElement>(`[${ROW_ATTRIBUTE}="${key}"]`) as HTMLElement,
  };
}

function board(overrides: Partial<MasterDetailOptions<number>> = {}) {
  let options: MasterDetailOptions<number> = {
    project: PROJECT,
    ledger: createNavigationLedger(),
    present: () => true,
    rowTrigger: (key) => `[${ROW_ATTRIBUTE}="${key}"]`,
    ...overrides,
  };

  const view = renderHook((props: MasterDetailOptions<number>) => useMasterDetail(props), {
    initialProps: options,
  });

  return {
    result: view.result,
    unmount: view.unmount,
    /** Re-renders the board the way a fresh snapshot does, with whatever has moved since. */
    refresh: (next: Partial<MasterDetailOptions<number>> = {}) => {
      options = { ...options, ...next };
      act(() => view.rerender(options));
    },
  };
}

describe("useMasterDetail", () => {
  it("hands the pane to a key's detail, telling the board before the route moves", () => {
    const fixture = panels([1, 2]);
    const opened: number[] = [];
    let keyWhenTold: number | null = -1;
    const view = board({
      onOpen: (key) => {
        opened.push(key);
        keyWhenTold = view.result.current.detailKey;
      },
    });

    act(() => view.result.current.open(1));

    expect(view.result.current.showing).toBe("detail");
    expect(view.result.current.detailKey).toBe(1);
    expect(opened).toEqual([1]);
    // Told while the board is still on whatever it was showing, so it can release what belonged
    // to that key before the panel it lived in goes inert.
    expect(keyWhenTold).toBeNull();
    // The list goes inert the moment the detail shows; focus left behind it would fall to the
    // document body and restart keyboard traversal at the top of the app.
    expect(document.activeElement).toBe(fixture.back);
  });

  it("keeps the key rendered until the panel leaving has settled, then releases it once", () => {
    const fixture = panels([1, 2]);
    let leaves = 0;
    const dropped: number[] = [];
    const view = board({
      onLeave: () => {
        leaves += 1;
      },
      onDrop: (key) => dropped.push(key),
    });
    act(() => view.result.current.open(1));

    act(() => view.result.current.back());

    expect(view.result.current.showing).toBe("list");
    // The panel is still sliding out and still carrying its subject, which is what keeps the
    // reader's content on screen for the length of that movement rather than blanking on the way.
    expect(view.result.current.detailKey).toBe(1);
    expect(dropped).toEqual([]);
    expect(leaves).toBe(1);
    expect(document.activeElement).toBe(fixture.row(1));

    act(() => view.result.current.onSettled());

    expect(view.result.current.detailKey).toBeNull();
    expect(dropped).toEqual([1]);
  });

  it("scrolls to a row it is returning to, and never to a panel that has just arrived", () => {
    // Both moves land focus; only one of them can be looking at something out of view. Asking the
    // page to scroll to a control at the top of an arriving panel buys nothing and costs a forced
    // layout of the whole page in the commit before that panel's first paint.
    const fixture = panels([1, 2]);
    const scrolled = vi.spyOn(Element.prototype, "scrollIntoView");
    const view = board();

    act(() => view.result.current.open(1));

    expect(document.activeElement).toBe(fixture.back);
    expect(scrolled).not.toHaveBeenCalled();

    act(() => view.result.current.back());

    expect(document.activeElement).toBe(fixture.row(1));
    expect(scrolled.mock.instances).toEqual([fixture.row(1)]);
    scrolled.mockRestore();
  });

  it("falls back to the list when the open key vanishes from the snapshot", () => {
    panels([1, 2]);
    const live = new Set([1, 2]);
    let leaves = 0;
    const dropped: number[] = [];
    const view = board({
      present: (key) => live.has(key),
      onLeave: () => {
        leaves += 1;
      },
      onDrop: (key) => dropped.push(key),
    });
    act(() => view.result.current.open(1));

    // Deleted, or moved out of this project, while its panel was up.
    live.delete(1);
    view.refresh();

    expect(view.result.current.showing).toBe("list");
    expect(view.result.current.detailKey).toBeNull();
    expect(leaves).toBe(1);
    expect(dropped).toEqual([1]);
  });

  it("opens the activation's target once it arrives in the snapshot", () => {
    const fixture = panels([1, 2]);
    const live = new Set<number>();
    // Mirrors a pane that mounts fresh and asks for a target before its first snapshot lands.
    const view = board({ present: (key) => live.has(key), focusKey: 2, focusNonce: 20 });
    expect(view.result.current.showing).toBe("list");

    live.add(2);
    view.refresh();

    expect(view.result.current.showing).toBe("detail");
    expect(view.result.current.detailKey).toBe(2);
    expect(document.activeElement).toBe(fixture.back);
  });

  it("opens on the list when an activation it already acted on is still standing at mount", () => {
    // The pane leaves the activation set on the props after acting on it and unmounts the board
    // whenever the user switches views, so switching away and back re-delivers the same one. A
    // nonce is one navigation, not a standing instruction to keep reopening the detail.
    panels([1, 2]);
    const ledger = createNavigationLedger();
    const first = board({ ledger, focusKey: 2, focusNonce: 30 });
    expect(first.result.current.showing).toBe("detail");
    first.unmount();

    const second = board({ ledger, focusKey: 2, focusNonce: 30 });

    expect(second.result.current.showing).toBe("list");
  });

  it("navigates on its own ledger, so another board's spent activation cannot silence it", () => {
    panels([1, 2]);
    const first = board({ ledger: createNavigationLedger(), focusKey: 2, focusNonce: 40 });
    expect(first.result.current.showing).toBe("detail");
    first.unmount();

    const second = board({ ledger: createNavigationLedger(), focusKey: 2, focusNonce: 40 });

    expect(second.result.current.showing).toBe("detail");
  });
});
