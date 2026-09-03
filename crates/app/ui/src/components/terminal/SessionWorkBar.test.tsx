// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render as renderRaw, waitFor } from "@testing-library/react";
import { SessionWorkBar } from "@/components/terminal/SessionWorkBar";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { SessionScratchpad, SessionTodo, SessionWork } from "@/domain";

afterEach(cleanup);

// The app mounts one TooltipProvider at its root; a bar rendered in isolation needs its own.
function render(ui: React.ReactNode) {
  return renderRaw(<TooltipProvider>{ui}</TooltipProvider>);
}

function todo(id: number, overrides: Partial<SessionTodo> = {}): SessionTodo {
  return {
    id,
    title: `todo ${id}`,
    status: "open",
    blocked: false,
    locked: false,
    access: "loaded",
    ...overrides,
  };
}

function pad(name: string): SessionScratchpad {
  return { id: name.length, name, access: "loaded" };
}

function work(overrides: Partial<SessionWork> = {}): SessionWork {
  return { process: 1, project: 1, todos: [], scratchpads: [], ...overrides };
}

const noop = () => {};

describe("SessionWorkBar", () => {
  it("renders nothing when there is no session work", () => {
    render(<SessionWorkBar work={null} onOpenTodo={noop} onOpenScratchpad={noop} />);
    expect(document.querySelector("[data-session-work]")).toBeNull();
  });

  it("renders nothing when the work has no todos or scratchpads", () => {
    render(<SessionWorkBar work={work()} onOpenTodo={noop} onOpenScratchpad={noop} />);
    expect(document.querySelector("[data-session-work]")).toBeNull();
  });

  it("puts a locked todo under Current work and a merely-touched one under This session", () => {
    render(
      <SessionWorkBar
        work={work({ todos: [todo(1, { locked: true }), todo(2, { locked: false })] })}
        onOpenTodo={noop}
        onOpenScratchpad={noop}
      />,
    );
    const current = document.querySelector('[data-session-group="current"]');
    const session = document.querySelector('[data-session-group="session"]');
    expect(current?.querySelector('[data-session-todo="1"]')).not.toBeNull();
    expect(session?.querySelector('[data-session-todo="2"]')).not.toBeNull();
    expect(current?.querySelector('[data-session-todo="2"]')).toBeNull();
  });

  it("keeps every item reachable past the inline limit, through the overflow control", async () => {
    const todos = [1, 2, 3, 4, 5].map((id) => todo(id, { locked: true }));
    render(<SessionWorkBar work={work({ todos })} onOpenTodo={noop} onOpenScratchpad={noop} />);

    const inlineCount = document.querySelectorAll(
      '[data-session-group="current"] [data-session-todo]',
    ).length;
    expect(inlineCount).toBeLessThan(5);

    const overflowTrigger = document.querySelector("[data-session-overflow]") as HTMLElement;
    expect(overflowTrigger).not.toBeNull();
    // The trigger opens on pointer down (Radix's `DropdownMenuTrigger`), not on `click`.
    fireEvent.pointerDown(overflowTrigger, { button: 0 });

    await waitFor(() => {
      expect(document.querySelectorAll("[data-session-todo]")).toHaveLength(5);
    });
  });

  it("truncates a long title and carries its full text in the tooltip", async () => {
    const long = "a very long todo title that should not fit inline without truncation";
    render(
      <SessionWorkBar
        work={work({ todos: [todo(1, { locked: true, title: long })] })}
        onOpenTodo={noop}
        onOpenScratchpad={noop}
      />,
    );
    const button = document.querySelector('[data-session-todo="1"]') as HTMLElement;
    const label = button.querySelector("span");
    expect(label?.className).toContain("truncate");

    fireEvent.focus(button);
    await waitFor(() => {
      const content = document.querySelector('[data-slot="tooltip-content"]');
      expect(content?.textContent).toContain(long);
    });
  });

  it("activates the right opener with the right id or name", () => {
    const onOpenTodo = vi.fn();
    const onOpenScratchpad = vi.fn();
    render(
      <SessionWorkBar
        work={work({
          todos: [todo(1, { locked: true })],
          scratchpads: [pad("plan/notes")],
        })}
        onOpenTodo={onOpenTodo}
        onOpenScratchpad={onOpenScratchpad}
      />,
    );

    fireEvent.click(document.querySelector('[data-session-todo="1"]') as HTMLElement);
    fireEvent.click(
      document.querySelector('[data-session-scratchpad="plan/notes"]') as HTMLElement,
    );

    expect(onOpenTodo).toHaveBeenCalledWith(1);
    expect(onOpenScratchpad).toHaveBeenCalledWith("plan/notes");
  });

  it("clips a group's own overflow and lets its items shrink, instead of spilling into a neighbour", () => {
    render(
      <SessionWorkBar
        work={work({
          todos: [todo(1, { locked: true, title: "a fairly long held todo title" })],
          scratchpads: [pad("a-fairly-long-scratchpad-name")],
        })}
        onOpenTodo={noop}
        onOpenScratchpad={noop}
      />,
    );

    // Each group clips its own content rather than letting a squeezed item paint over its
    // sibling group or the process controls.
    const currentTokens = document
      .querySelector('[data-session-group="current"]')
      ?.className.split(/\s+/);
    const sessionTokens = document
      .querySelector('[data-session-group="session"]')
      ?.className.split(/\s+/);
    expect(currentTokens).toContain("overflow-hidden");
    expect(sessionTokens).toContain("overflow-hidden");

    // An item button shrinks with its row rather than holding the shadcn `Button` default of
    // `shrink-0`, which would force it to keep its full content width regardless of available space.
    const button = document.querySelector('[data-session-todo="1"]') as HTMLElement;
    const buttonTokens = button.className.split(/\s+/);
    expect(buttonTokens).toContain("shrink");
    expect(buttonTokens).not.toContain("shrink-0");
    expect(buttonTokens).toContain("min-w-0");

    // The truncating label itself needs `min-w-0` to actually shrink inside the button — a
    // `truncate` class alone has no effect on a flex child whose min-width defaults to its content.
    const label = button.querySelector("span");
    const labelTokens = label?.className.split(/\s+/) ?? [];
    expect(labelTokens).toContain("min-w-0");
    expect(labelTokens).toContain("truncate");
  });

  it("renders items as keyboard-reachable buttons with a visible focus ring", () => {
    render(
      <SessionWorkBar
        work={work({ todos: [todo(1, { locked: true })] })}
        onOpenTodo={noop}
        onOpenScratchpad={noop}
      />,
    );
    const button = document.querySelector('[data-session-todo="1"]') as HTMLButtonElement;
    expect(button.tagName).toBe("BUTTON");
    expect(button.tabIndex).not.toBe(-1);
    expect(button.className).toMatch(/focus-visible:ring/);
  });
});
