// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { cleanup, render, waitFor } from "@testing-library/react";

import { DiffViewer, SIDE_BY_SIDE, UNIFIED } from "@/components/git/DiffViewer";
import type { FileDiff } from "@/domain";

const PATH = "src/main.rs";

const PATCH = `diff --git a/${PATH} b/${PATH}
--- a/${PATH}
+++ b/${PATH}
@@ -1,3 +1,3 @@
 fn main() {
-    let old = 1;
+    let renamed = 2;
 }
`;

function diffOf(path = PATH): FileDiff {
  return {
    path,
    original_path: null,
    target: "unstaged",
    binary: false,
    patch: PATCH.split(PATH).join(path),
    hunks: [{ old_start: 1, old_lines: 3, new_start: 1, new_lines: 3 }],
    truncated: false,
  };
}

// The viewer measures text on a canvas to size its gutter, which jsdom has no implementation
// for. Answering with a width per character is enough for it to lay out and render; nothing here
// asserts a measurement.
beforeAll(() => {
  HTMLCanvasElement.prototype.getContext = (() => ({
    font: "",
    measureText: (text: string) => ({ width: text.length * 8 }),
  })) as unknown as HTMLCanvasElement["getContext"];
});

afterEach(cleanup);

/** The rendered text, joined — a highlighted line is split across a span per token. */
async function shown(container: HTMLElement, text: string): Promise<void> {
  await waitFor(() => expect(container.textContent).toContain(text));
}

describe("DiffViewer", () => {
  it("renders the lines the patch changed", async () => {
    const { container } = render(<DiffViewer diff={diffOf()} layout={UNIFIED} dark={false} />);

    await shown(container, "let renamed = 2;");
    await shown(container, "let old = 1;");
  });

  it("colours the code through the app's own highlighter", async () => {
    const { container } = render(<DiffViewer diff={diffOf()} layout={UNIFIED} dark={false} />);
    await shown(container, "let renamed = 2;");

    // Every token carries a colour for each theme, which is what lets a light/dark flip recolour
    // what is already on screen. Finding one proves the whole chain ran: the grammar was fetched,
    // the highlighter produced a tree, and the viewer rendered it.
    await waitFor(() =>
      expect(
        container.querySelector('[style*="--diff-view-light"]'),
        "no token carried a highlighted colour",
      ).not.toBeNull(),
    );
  });

  it("shows a file it has no grammar for without colouring it as something else", async () => {
    const { container } = render(
      <DiffViewer diff={diffOf("notes.unknownext")} layout={UNIFIED} dark={false} />,
    );

    await shown(container, "let renamed = 2;");
    expect(
      container.querySelector('[style*="--diff-view-light"]'),
      "nothing coloured it, because there is no grammar for it",
    ).toBeNull();
    expect(
      container.querySelector('[class*="hljs-"]'),
      "and nothing guessed at it: colouring is asked for only once a grammar is in place",
    ).toBeNull();
  });

  it("lays the two sides out in their own columns", async () => {
    const unified = render(<DiffViewer diff={diffOf()} layout={UNIFIED} dark={false} />);
    await shown(unified.container, "let renamed = 2;");
    const unifiedRows = unified.container.querySelectorAll("tr").length;
    cleanup();

    const split = render(<DiffViewer diff={diffOf()} layout={SIDE_BY_SIDE} dark={false} />);
    await shown(split.container, "let renamed = 2;");

    // Unified stacks the removed line above the added one; side by side puts them on one row, so
    // the same patch occupies fewer rows.
    expect(split.container.querySelectorAll("tr").length).toBeLessThan(unifiedRows);
  });

  it("renders the actions for a hunk beside the hunk itself", async () => {
    const { container } = render(
      <DiffViewer
        diff={diffOf()}
        layout={UNIFIED}
        dark={false}
        actions={(hunk) => (
          <button type="button">{`stage ${hunk.old_start}-${hunk.new_start}`}</button>
        )}
      />,
    );

    await shown(container, "let renamed = 2;");
    // The hunk the patch declares, not the row it happened to land on: the viewer is free to
    // mount and unmount rows, and an action tied to a position would follow the wrong change.
    await waitFor(() => expect(container.textContent).toContain("stage 1-1"));
  });

  it("renders no actions at all when a reader may not act on the change", async () => {
    const { container } = render(<DiffViewer diff={diffOf()} layout={UNIFIED} dark={false} />);

    await shown(container, "let renamed = 2;");
    expect(container.querySelector("button")).toBeNull();
  });
});
