// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TodoItem } from "@/components/orchestration/TodoItem";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ScratchpadRef, TodoStatus, TodoView } from "@/domain";

afterEach(cleanup);

const plan: ScratchpadRef = { id: 4, name: "release-plan" };

const STATUSES: TodoStatus[] = ["open", "in_progress", "blocked", "done"];

function todo(overrides: Partial<TodoView> = {}): TodoView {
  return {
    id: 1,
    doc: { title: "Ship the release", body: "", status: "open" },
    tags: [],
    blockers: [],
    blocked_by: [],
    blocked: false,
    comments: [],
    locked_by: null,
    scratchpad: null,
    revision: 1,
    ...overrides,
  };
}

function row(overrides: Partial<Parameters<typeof TodoItem>[0]> = {}) {
  return render(
    <TooltipProvider delayDuration={0}>
      <TodoItem todo={todo()} onOpen={vi.fn()} lockOwnerLabel={undefined} {...overrides} />
    </TooltipProvider>,
  );
}

describe("TodoItem", () => {
  it("renders the row's tags", () => {
    row({ todo: todo({ tags: ["a", "b"] }) });

    expect(screen.getByText("a")).toBeTruthy();
    expect(screen.getByText("b")).toBeTruthy();
    const tags = document.querySelector("[data-tags]") as HTMLElement;
    expect(tags.parentElement?.getAttribute("data-todo-tag-row")).not.toBeNull();
    expect(tags.className).toContain("flex-wrap");
  });

  it("composes the work item from the shared card, button and badge primitives", () => {
    row({
      todo: todo({
        tags: ["release"],
        blockers: [2],
        blocked_by: [2],
        comments: [{ id: 1, body: "Ready", author: null }],
      }),
    });

    expect(document.querySelector("[data-todo-card]")?.getAttribute("data-slot")).toBe("card");
    expect(document.querySelector("[data-todo-trigger]")?.getAttribute("data-slot")).toBe("button");
    for (const selector of [
      "[data-todo-status]",
      "[data-todo-blockers]",
      "[data-todo-comments]",
      "[data-tag]",
    ]) {
      expect(document.querySelector(selector)?.getAttribute("data-slot")).toBe("badge");
    }
  });

  it("uses one flush, geometry-stable surface for the card action", () => {
    row();

    const content = document.querySelector('[data-slot="card-content"]') as HTMLElement;
    const trigger = document.querySelector("[data-todo-trigger]") as HTMLElement;

    expect(content.className).toContain("p-0");
    expect(content.className).toContain("gap-0");
    expect(trigger.className).toContain("rounded-none");
    expect(trigger.className).toContain("active:not-aria-[haspopup]:scale-100");
    expect(trigger.className).not.toContain("scale-[0.97]");
    expect(trigger.className).toContain("focus-visible:ring-inset");
  });

  it("yields the title alone on the first line, so the status label is never clipped", () => {
    row({ todo: todo({ blocked_by: [2], blockers: [2], locked_by: 9 }), onOpenAgent: vi.fn() });

    const trigger = document.querySelector("[data-todo-trigger]") as HTMLElement;
    expect(trigger.className).toContain("overflow-hidden");

    const title = document.querySelector("[data-todo-title]") as HTMLElement;
    expect(title.className).toContain("min-w-0");
    expect(title.className).toContain("truncate");

    // A declared status is one of four fixed strings; clipped to "In prog…" it stops being
    // readable, so the chip holds its natural width and its label never truncates.
    const status = document.querySelector("[data-todo-status]") as HTMLElement;
    expect(status.className).toContain("shrink-0");
    expect(status.querySelector("svg")?.getAttribute("data-icon")).toBe("inline-start");
    expect(status.querySelector("span")?.className).not.toContain("truncate");

    // The meta line still clips, so it cannot spill under the sibling agent control.
    const blockers = document.querySelector("[data-todo-blockers]") as HTMLElement;
    expect(blockers.className).toContain("min-w-0");
    expect(blockers.querySelector("span")?.className).toContain("truncate");
  });

  it("renders its id and its status label", () => {
    row();

    expect(screen.getByText("#1")).toBeTruthy();
    expect(screen.getByText("Open")).toBeTruthy();
  });

  it("never names the scratchpad it derives from, which the board already groups by", () => {
    row({ todo: todo({ scratchpad: plan }) });

    expect(screen.queryByText("Release plan")).toBeNull();
  });

  it("gives each declared status its own tone, so no two read alike", () => {
    const tones = STATUSES.map((status) => {
      cleanup();
      row({ todo: todo({ doc: { title: "Ship the release", body: "", status } }) });
      const chip = document.querySelector("[data-todo-status]") as HTMLElement;
      expect(chip.dataset.status).toBe(status);
      // The tone dresses the chip, never the label: a `--status-*` hue is only guaranteed 3:1, and
      // the built-in light palette's amber measures 2.5:1 — unreadable as text.
      const label = chip.querySelector("span") as HTMLElement;
      expect(label.className).toContain("text-foreground");
      return [...chip.classList].filter((name) => name.startsWith("text-")).join(" ");
    });

    expect(tones.every((tone) => tone !== "")).toBe(true);
    expect(new Set(tones).size).toBe(STATUSES.length);
  });

  it("names a blocked row's unmet-blocker count with the right plurality", () => {
    row({ todo: todo({ blockers: [2, 3], blocked_by: [2, 3], blocked: true }) });

    expect(screen.getByText("2 unmet blockers")).toBeTruthy();
  });

  it("renders no blocker text once nothing is unmet", () => {
    row({ todo: todo({ blockers: [2], blocked_by: [], blocked: false }) });

    expect(screen.queryByText(/unmet blocker/)).toBeNull();
  });

  it("opens the todo when the card is activated", () => {
    const onOpen = vi.fn();
    row({ onOpen });

    fireEvent.click(document.querySelector("[data-todo-trigger]") as HTMLElement);

    expect(onOpen).toHaveBeenCalled();
  });

  it("keeps the card's own button free of any interactive descendant", () => {
    row({ todo: todo({ locked_by: 9 }), onOpenAgent: vi.fn() });

    const trigger = document.querySelector("[data-todo-trigger]") as HTMLElement;
    expect(trigger.querySelector('button, [role="button"], a[href]')).toBeNull();
  });

  it("activates the agent control without opening the todo", () => {
    const onOpenAgent = vi.fn();
    const onOpen = vi.fn();
    row({ todo: todo({ locked_by: 9 }), lockOwnerLabel: "worker-a", onOpenAgent, onOpen });

    fireEvent.click(screen.getByRole("button", { name: "Open worker-a terminal" }));

    expect(onOpenAgent).toHaveBeenCalledWith(9);
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("caps a long agent name and truncates only its visible label", () => {
    const label = "release-coordinator-with-an-unusually-long-process-name";
    row({ todo: todo({ locked_by: 9 }), lockOwnerLabel: label, onOpenAgent: vi.fn() });

    const agent = screen.getByRole("button", { name: `Open ${label} terminal` });
    const visibleLabel = agent.querySelector("[data-todo-agent-label]") as HTMLElement;

    expect(agent.className).toContain("max-w-");
    expect(agent.className).toContain("min-w-0");
    expect(visibleLabel.textContent).toBe(label);
    expect(visibleLabel.className).toContain("min-w-0");
    expect(visibleLabel.className).toContain("truncate");
  });

  it("explains the agent navigation action when its control receives focus", async () => {
    row({ todo: todo({ locked_by: 9 }), lockOwnerLabel: "worker-a", onOpenAgent: vi.fn() });

    fireEvent.focus(document.querySelector("[data-todo-agent]") as HTMLElement);

    expect((await screen.findByRole("tooltip")).textContent).toBe("Open worker-a terminal");
  });

  it("is keyboard-reachable as its own tab stop, separate from the card's button", () => {
    row({ todo: todo({ locked_by: 9 }), onOpenAgent: vi.fn() });

    const trigger = document.querySelector("[data-todo-trigger]") as HTMLElement;
    const agent = document.querySelector("[data-todo-agent]") as HTMLElement;
    expect(agent.tagName).toBe("BUTTON");
    expect(agent.tabIndex).not.toBe(-1);
    expect(trigger.contains(agent)).toBe(false);
  });

  it("renders the agent control disabled when no onOpenAgent is given", () => {
    row({ todo: todo({ locked_by: 9 }) });

    const agent = document.querySelector("[data-todo-agent]") as HTMLButtonElement;
    expect(agent.disabled).toBe(true);
  });

  it("renders no agent control on an unlocked row", () => {
    row({ todo: todo({ locked_by: null }), onOpenAgent: vi.fn() });

    expect(document.querySelector("[data-todo-agent]")).toBeNull();
  });
});
