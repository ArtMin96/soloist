// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { Editor } from "@tiptap/react";
import { EditorView } from "@tiptap/pm/view";
import RichTextEditor, { type RichTextEditorProps } from "@/components/editor/RichTextEditor";

/** The editor's own DOM node — the element ProseMirror owns and measures. */
const EDITOR_SELECTOR = '[data-editor="rich-text"]';

/**
 * How often the view's props may be applied while one editor comes up: TipTap hands the view its
 * element and then its node views, and each of those applies props once. Anything beyond that is a
 * re-application, and every one of them costs a full style and layout pass of the page.
 */
const MOUNT_PROP_APPLICATIONS = 2;

const NOTE = "# Title\n\nA paragraph.";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function readOnlyEditor(onChange: (markdown: string) => void) {
  const props: RichTextEditorProps = {
    initialMarkdown: NOTE,
    onChange,
    editable: false,
    toolbar: false,
    slash: false,
    ariaLabel: "note body",
  };
  return <RichTextEditor {...props} />;
}

/** Waits until the editor has mounted and its Markdown has been seeded, then hands back its DOM. */
async function seededEditorDom(): Promise<HTMLElement> {
  await screen.findByText("Title");
  const dom = document.querySelector(EDITOR_SELECTOR);
  if (!(dom instanceof HTMLElement)) throw new Error(`no ${EDITOR_SELECTOR} in the document`);
  return dom;
}

/** Lets the effects and subscriptions a mount schedules run to quiet before counts are read. */
async function settle(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
  });
}

describe("RichTextEditor", () => {
  it("re-renders without re-applying the editor's options", async () => {
    const setOptions = vi.spyOn(Editor.prototype, "setOptions");
    const { rerender } = render(readOnlyEditor(() => {}));
    await seededEditorDom();
    await settle();

    const appliedOnMount = setOptions.mock.calls.length;
    for (let pass = 0; pass < 3; pass += 1) rerender(readOnlyEditor(() => {}));
    await settle();

    expect(setOptions.mock.calls.length).toBe(appliedOnMount);
  });

  it("applies the view's props no more often than bringing one up requires", async () => {
    const setProps = vi.spyOn(EditorView.prototype, "setProps");
    render(readOnlyEditor(() => {}));
    await seededEditorDom();
    await settle();

    expect(setProps.mock.calls.length).toBeLessThanOrEqual(MOUNT_PROP_APPLICATIONS);
  });

  // ProseMirror measures and hit-tests the editor around every redraw to hold the viewport steady,
  // unless the editor node carries `overflow-anchor`. It reads the JS property, so the opt-out is
  // assigned rather than declared — and it is scoped to a document with no caret to hold in place.
  it("opts a read-only document out of ProseMirror's scroll anchoring", async () => {
    render(readOnlyEditor(() => {}));
    const dom = await seededEditorDom();

    expect(dom.style.overflowAnchor).toBe("none");
  });

  it("leaves an editable document's scroll anchoring in place", async () => {
    render(
      <RichTextEditor
        initialMarkdown={NOTE}
        onChange={() => {}}
        toolbar={false}
        slash={false}
        ariaLabel="note body"
      />,
    );
    const dom = await seededEditorDom();

    expect(dom.style.overflowAnchor).not.toBe("none");
  });
});
