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
});
