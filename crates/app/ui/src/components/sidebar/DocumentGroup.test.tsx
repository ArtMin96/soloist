// @vitest-environment jsdom
import { CircleDashedIcon } from "lucide-react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { DocumentGroup } from "@/components/sidebar/DocumentGroup";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { DocumentItem } from "@/components/sidebar/documentItems";
import type { DocumentParticipant } from "@/domain";

function participant(process: number, role: DocumentParticipant["role"]): DocumentParticipant {
  return { process, label: `agent-${process}`, role };
}

function item(overrides: Partial<DocumentItem> = {}): DocumentItem {
  return {
    key: "todo-1",
    label: "wire the header",
    icon: CircleDashedIcon,
    tone: "text-status-stopped",
    role: "reading",
    participants: [participant(9, "reading")],
    handle: 1,
    activate: vi.fn(),
    ...overrides,
  };
}

const noop = () => {};

function renderGroup(items: DocumentItem[], section: "todos" | "scratchpads" = "todos") {
  render(
    <TooltipProvider delayDuration={0}>
      <DocumentGroup section={section} items={items} open onOpenChange={noop} />
    </TooltipProvider>,
  );
}

afterEach(cleanup);

describe("DocumentGroup", () => {
  it("shows the section's label and a count equal to its item count", () => {
    renderGroup([item(), item({ key: "todo-2", handle: 2 })]);
    expect(screen.getByText("Todos")).toBeTruthy();
    expect(screen.getByText("2")).toBeTruthy();
  });

  it("carries data-document-section on its list", () => {
    renderGroup([item()], "scratchpads");
    const list = document.querySelector('[data-document-section="scratchpads"]');
    expect(list).not.toBeNull();
    expect(list?.getAttribute("aria-label")).toBe("Scratchpads");
  });

  it("renders every item as a document row", () => {
    renderGroup([item({ label: "first" }), item({ key: "todo-2", label: "second", handle: 2 })]);
    expect(screen.getAllByRole("button", { name: /first|second/ })).toHaveLength(2);
  });

  it("contains no element with role treeitem — the guard for keyboard traversal staying on process rows", () => {
    renderGroup([item()]);
    expect(screen.queryByRole("treeitem")).toBeNull();
  });

  it("renders an empty list when given no items", () => {
    renderGroup([]);
    const list = document.querySelector('[data-document-section="todos"]');
    expect(list?.children).toHaveLength(0);
  });
});
