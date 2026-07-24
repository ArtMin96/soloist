// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { DiagramPanel } from "@/components/orchestration/DiagramPanel";

// The panel reaches IPC only when a diagram is opened; an empty roster opens nothing, but the module
// still imports the typed API, so it is mocked to keep the Tauri bridge out of the headless test.
vi.mock("@/api", () => ({
  diagramRead: vi.fn(),
  diagramWrite: vi.fn(),
  diagramRename: vi.fn(),
  diagramArchive: vi.fn(),
  exportDiagramFile: vi.fn(),
}));

afterEach(cleanup);

describe("DiagramPanel", () => {
  it("shows the first-run guidance and the pick-one placeholder when there are no diagrams", () => {
    render(<DiagramPanel project={7} diagrams={[]} />);
    expect(screen.getByText(/No diagrams yet/)).toBeTruthy();
    expect(screen.getByText(/Select a diagram to read or edit it/)).toBeTruthy();
  });
});
