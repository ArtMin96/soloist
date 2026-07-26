// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import RichTextEditor from "./RichTextEditor";

// The NodeView draws its diagram through the engine; mock that boundary so the editor mounts without
// loading the real Mermaid library, and the presence of the canned svg becomes the "preview is
// showing" signal the toggle assertions key off. React node views only render through the editor's
// `EditorContent`, so the test drives the real editor component rather than a bare `Editor`.
vi.mock("@/lib/mermaid/engine", () => ({
  renderDiagram: vi.fn(() => Promise.resolve({ svg: "<svg data-testid='mmd-diagram'></svg>" })),
  parseDiagram: vi.fn(() => Promise.resolve({ ok: true })),
}));

function mountEditor(markdown: string) {
  return render(
    <RichTextEditor initialMarkdown={markdown} onChange={() => {}} toolbar={false} slash={false} />,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("MermaidCodeBlockView", () => {
  it("leaves an ordinary code block untouched — no diagram, no header", async () => {
    const { container } = mountEditor("```ts\nconst x = 1;\n```");

    await waitFor(() =>
      expect(container.querySelector("pre code")?.textContent).toContain("const x = 1;"),
    );
    expect(screen.queryByTestId("mmd-diagram")).toBeNull();
    expect(screen.queryByText("Mermaid")).toBeNull();
  });

  it("renders a mermaid fence as a diagram under a Mermaid header", async () => {
    mountEditor("```mermaid\nflowchart TD\n  A --> B\n```");

    expect(await screen.findByTestId("mmd-diagram")).toBeTruthy();
    expect(screen.getByText("Mermaid")).toBeTruthy();
  });

  it("swaps the diagram for the editable source when Source is chosen", async () => {
    mountEditor("```mermaid\nflowchart TD\n  A --> B\n```");
    await screen.findByTestId("mmd-diagram");

    fireEvent.click(screen.getByText("Source"));

    await waitFor(() => expect(screen.queryByTestId("mmd-diagram")).toBeNull());
    expect(document.body.textContent).toContain("flowchart TD");
  });
});
