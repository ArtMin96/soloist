// @vitest-environment jsdom
import { CircleDotIcon } from "lucide-react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DocumentRow } from "@/components/sidebar/DocumentRow";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { DocumentItem } from "@/components/sidebar/documentItems";
import type { DocumentParticipant } from "@/domain";

function participant(process: number, role: DocumentParticipant["role"]): DocumentParticipant {
  return { process, label: `agent-${process}`, role };
}

function todoItem(overrides: Partial<DocumentItem> = {}): DocumentItem {
  return {
    key: "todo-1",
    label: "Ship the sidebar groups",
    icon: CircleDotIcon,
    tone: "text-status-running",
    role: "implementing",
    participants: [participant(3, "implementing")],
    handle: 1,
    activate: vi.fn(),
    ...overrides,
  };
}

function renderRow(item: DocumentItem, section: "todos" | "scratchpads" = "todos") {
  render(
    <TooltipProvider delayDuration={0}>
      <ul>
        <DocumentRow item={item} section={section} />
      </ul>
    </TooltipProvider>,
  );
}

afterEach(cleanup);

describe("DocumentRow", () => {
  it("exposes the role through the accessible name and data-document-role, never as visible text", () => {
    renderRow(todoItem());
    const button = screen.getByRole("button");
    expect(button.getAttribute("aria-label")).toContain("Implementing");
    expect(button.getAttribute("data-document-role")).toBe("implementing");
    expect(screen.queryByText("Implementing")).toBeNull();
  });

  it("carries the todo id as data-document-todo", () => {
    renderRow(todoItem({ handle: 42 }));
    expect(screen.getByRole("button").getAttribute("data-document-todo")).toBe("42");
    expect(screen.getByRole("button").getAttribute("data-document-scratchpad")).toBeNull();
  });

  it("carries the scratchpad id as data-document-scratchpad", () => {
    renderRow(todoItem({ handle: 4 }), "scratchpads");
    expect(screen.getByRole("button").getAttribute("data-document-scratchpad")).toBe("4");
    expect(screen.getByRole("button").getAttribute("data-document-todo")).toBeNull();
  });

  it("clicking calls activate exactly once", () => {
    const activate = vi.fn();
    renderRow(todoItem({ activate }));
    fireEvent.click(screen.getByRole("button"));
    expect(activate).toHaveBeenCalledTimes(1);
  });

  it("a single-participant row shows no count and names one agent's accessible state", () => {
    renderRow(todoItem({ participants: [participant(3, "implementing")] }));
    const button = screen.getByRole("button");
    expect(button.getAttribute("aria-label")).toBe("Ship the sidebar groups, Implementing");
    expect(screen.queryByText(/×/)).toBeNull();
  });

  it("a two-participant row shows the count and its accessible name says how many agents", () => {
    renderRow(
      todoItem({
        participants: [participant(3, "implementing"), participant(5, "reading")],
      }),
    );
    expect(screen.getByText("×2")).toBeTruthy();
    expect(screen.getByRole("button").getAttribute("aria-label")).toBe(
      "Ship the sidebar groups, Implementing, 2 agents",
    );
  });

  it("the tooltip content names every participant", async () => {
    renderRow(
      todoItem({
        participants: [participant(3, "implementing"), participant(5, "reading")],
      }),
    );
    fireEvent.focus(screen.getByRole("button"));
    const matches = await screen.findAllByText("agent-3 — Implementing, agent-5 — Reading");
    expect(matches.length).toBeGreaterThan(0);
  });
});
