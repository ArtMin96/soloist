// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { RemoveProcessDialog } from "@/components/RemoveProcessDialog";
import type { ProcessView } from "@/domain";

const CLAUDE: ProcessView = {
  id: 3,
  project: 1,
  kind: "Agent",
  label: "Claude",
  status: "Running",
  exit_code: null,
  requires_trust: false,
  resumable: true,
  ports: [],
  ready: "Ungated",
};

afterEach(cleanup);

describe("RemoveProcessDialog", () => {
  it("stays closed while no process is held for confirmation", () => {
    render(
      <RemoveProcessDialog process={null} workers={0} onConfirm={vi.fn()} onDismiss={vi.fn()} />,
    );
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("names the process it would remove", () => {
    render(
      <RemoveProcessDialog process={CLAUDE} workers={0} onConfirm={vi.fn()} onDismiss={vi.fn()} />,
    );
    expect(screen.getByText("Remove “Claude”?")).toBeTruthy();
  });

  it("states that the output is discarded and the project's files are not", () => {
    render(
      <RemoveProcessDialog process={CLAUDE} workers={0} onConfirm={vi.fn()} onDismiss={vi.fn()} />,
    );
    expect(screen.getByText("its output — the scrollback is not saved anywhere")).toBeTruthy();
    expect(screen.getByText("every file it wrote in the project, untouched")).toBeTruthy();
  });

  it("removes only when the destructive action is chosen", () => {
    const onConfirm = vi.fn();
    render(
      <RemoveProcessDialog
        process={CLAUDE}
        workers={0}
        onConfirm={onConfirm}
        onDismiss={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("dismisses without removing when cancelled", () => {
    const onConfirm = vi.fn();
    const onDismiss = vi.fn();
    render(
      <RemoveProcessDialog
        process={CLAUDE}
        workers={0}
        onConfirm={onConfirm}
        onDismiss={onDismiss}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onConfirm).not.toHaveBeenCalled();
    expect(onDismiss).toHaveBeenCalled();
  });

  it("says nothing about workers when the process spawned none", () => {
    render(
      <RemoveProcessDialog process={CLAUDE} workers={0} onConfirm={vi.fn()} onDismiss={vi.fn()} />,
    );
    expect(screen.queryByText(/spawned/)).toBeNull();
  });

  it("says the agents it spawned keep running, because removing a lead does not stop them", () => {
    // A worker is a separate managed process in its own group, so the lead's reap never reaches
    // it. A dialog that implied otherwise would be asking the user to agree to something false.
    render(
      <RemoveProcessDialog process={CLAUDE} workers={2} onConfirm={vi.fn()} onDismiss={vi.fn()} />,
    );
    expect(screen.getByText("the 2 agents it spawned still running, on their own")).toBeTruthy();
  });

  it("counts a single worker in the singular", () => {
    render(
      <RemoveProcessDialog process={CLAUDE} workers={1} onConfirm={vi.fn()} onDismiss={vi.fn()} />,
    );
    expect(screen.getByText("the 1 agent it spawned still running, on its own")).toBeTruthy();
  });

  it("never claims to stop more than this process and its own children", () => {
    render(
      <RemoveProcessDialog process={CLAUDE} workers={3} onConfirm={vi.fn()} onDismiss={vi.fn()} />,
    );
    expect(screen.getByText("this process and the child processes it started")).toBeTruthy();
  });

  it("offers no close X, so the only ways out are the two stated choices", () => {
    // The choice is the dialog's whole job; an X would be a third, unlabelled exit.
    render(
      <RemoveProcessDialog process={CLAUDE} workers={0} onConfirm={vi.fn()} onDismiss={vi.fn()} />,
    );
    expect(screen.queryByRole("button", { name: /close/i })).toBeNull();
  });
});
