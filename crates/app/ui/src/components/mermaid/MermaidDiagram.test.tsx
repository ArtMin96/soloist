// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { MermaidDiagram } from "./MermaidDiagram";
import { renderDiagram } from "@/lib/mermaid/engine";

// Only the library boundary is mocked; the theme hook runs for real (jsdom has MutationObserver), so
// the component's own render/state logic is what these assertions exercise.
vi.mock("@/lib/mermaid/engine", () => ({ renderDiagram: vi.fn() }));

const renderDiagramMock = vi.mocked(renderDiagram);

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("MermaidDiagram", () => {
  it("shows the skeleton while a render is in flight", () => {
    renderDiagramMock.mockReturnValue(new Promise(() => {}));

    render(<MermaidDiagram source="flowchart TD\n A --> B" />);

    expect(screen.getByTestId("mermaid-skeleton")).toBeTruthy();
  });

  it("injects the rendered svg and reports the source as valid", async () => {
    renderDiagramMock.mockResolvedValue({ svg: "<svg data-testid='diagram-svg'></svg>" });
    const onParse = vi.fn();

    render(<MermaidDiagram source="flowchart TD\n A --> B" onParse={onParse} />);

    expect(await screen.findByTestId("diagram-svg")).toBeTruthy();
    await waitFor(() => expect(onParse).toHaveBeenCalledWith(true));
  });

  it("shows an icon-and-label error banner and reports the source as invalid", async () => {
    renderDiagramMock.mockResolvedValue({ error: "Parse error on line 2" });
    const onParse = vi.fn();

    const { container } = render(<MermaidDiagram source="not a diagram" onParse={onParse} />);

    const banner = await screen.findByRole("alert");
    expect(banner.textContent).toContain("Parse error on line 2");
    // The banner carries the warning icon, so the state reads without relying on color alone.
    expect(container.querySelector(".mermaid-error-icon")).toBeTruthy();
    await waitFor(() => expect(onParse).toHaveBeenCalledWith(false));
  });

  it("keeps the drawn diagram on screen while the next render is in flight", async () => {
    renderDiagramMock.mockResolvedValue({ svg: "<svg data-testid='first'></svg>" });
    const { rerender, container } = render(<MermaidDiagram source="flowchart TD\n A --> B" />);
    expect(await screen.findByTestId("first")).toBeTruthy();

    // A theme pick rewrites the source; the render behind it has not resolved yet.
    renderDiagramMock.mockReturnValue(new Promise(() => {}));
    rerender(<MermaidDiagram source="---\nconfig:\n theme: dark\n---\nflowchart TD\n A --> B" />);

    // A Mermaid render is slow enough that swapping to a placeholder reads as the diagram vanishing.
    expect(screen.queryByTestId("mermaid-skeleton")).toBeNull();
    expect(screen.getByTestId("first")).toBeTruthy();
    await waitFor(() => expect(container.querySelector(".mermaid-rendered.is-stale")).toBeTruthy());
  });

  it("keeps the last diagram that drew when an edit breaks the source", async () => {
    renderDiagramMock.mockResolvedValue({ svg: "<svg data-testid='drawn'></svg>" });
    const { rerender, container } = render(<MermaidDiagram source="flowchart TD\n A --> B" />);
    expect(await screen.findByTestId("drawn")).toBeTruthy();

    renderDiagramMock.mockResolvedValue({ error: "Parse error on line 2" });
    rerender(<MermaidDiagram source="flowchart TD\n A -->" />);

    expect(await screen.findByRole("alert")).toBeTruthy();
    // Still there to work against, but dimmed — it no longer reflects what is in the editor.
    expect(screen.getByTestId("drawn")).toBeTruthy();
    expect(container.querySelector(".mermaid-rendered.is-stale")).toBeTruthy();
  });

  it("clears the stale marking once the new diagram has drawn", async () => {
    renderDiagramMock.mockResolvedValue({ svg: "<svg data-testid='first'></svg>" });
    const { rerender, container } = render(<MermaidDiagram source="flowchart TD\n A --> B" />);
    expect(await screen.findByTestId("first")).toBeTruthy();

    renderDiagramMock.mockResolvedValue({ svg: "<svg data-testid='second'></svg>" });
    rerender(<MermaidDiagram source="flowchart LR\n A --> B" />);

    expect(await screen.findByTestId("second")).toBeTruthy();
    expect(container.querySelector(".mermaid-rendered.is-stale")).toBeNull();
  });
});
