// @vitest-environment jsdom
import { describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useProcessRemoval } from "@/store/useProcessRemoval";
import type { ProcessView } from "@/domain";

function process(overrides: Partial<ProcessView> = {}): ProcessView {
  return {
    id: 1,
    project: 1,
    kind: "Terminal",
    label: "Terminal",
    status: "Stopped",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
    ...overrides,
  };
}

function setup(processes: ProcessView[]) {
  const close = vi.fn();
  const view = renderHook(({ list }: { list: ProcessView[] }) => useProcessRemoval(list, close), {
    initialProps: { list: processes },
  });
  return { close, view };
}

describe("useProcessRemoval", () => {
  it("removes a resting process immediately, with nothing to confirm", () => {
    const { close, view } = setup([process({ id: 4, status: "Stopped" })]);
    act(() => view.result.current.request(4));
    expect(close).toHaveBeenCalledWith(4);
    expect(view.result.current.pending).toBeNull();
  });

  it.each(["Crashed", "RestartExhausted"] as const)(
    "removes a %s process immediately — it holds no child to kill",
    (status) => {
      const { close, view } = setup([process({ id: 4, status })]);
      act(() => view.result.current.request(4));
      expect(close).toHaveBeenCalledWith(4);
      expect(view.result.current.pending).toBeNull();
    },
  );

  it("holds a live process for confirmation instead of removing it", () => {
    const { close, view } = setup([process({ id: 4, status: "Running", label: "Claude" })]);
    act(() => view.result.current.request(4));
    expect(close).not.toHaveBeenCalled();
    expect(view.result.current.pending?.label).toBe("Claude");
  });

  it("removes the held process only once confirmed", () => {
    const { close, view } = setup([process({ id: 4, status: "Running" })]);
    act(() => view.result.current.request(4));
    act(() => view.result.current.confirm());
    expect(close).toHaveBeenCalledWith(4);
    expect(view.result.current.pending).toBeNull();
  });

  it("leaves the process alone when the confirmation is dismissed", () => {
    const { close, view } = setup([process({ id: 4, status: "Running" })]);
    act(() => view.result.current.request(4));
    act(() => view.result.current.dismiss());
    expect(close).not.toHaveBeenCalled();
    expect(view.result.current.pending).toBeNull();
  });

  it("keeps the confirmation open when the held process exits on its own", () => {
    // The intent still makes sense — the row is there either way — so the dialog stays and
    // confirming still clears it, rather than making the user start over.
    const { close, view } = setup([process({ id: 4, status: "Running" })]);
    act(() => view.result.current.request(4));
    view.rerender({ list: [process({ id: 4, status: "Stopped" })] });
    expect(view.result.current.pending?.id).toBe(4);
    act(() => view.result.current.confirm());
    expect(close).toHaveBeenCalledWith(4);
  });

  it("drops the confirmation when the held process is removed by another surface", () => {
    // An agent closing it over MCP leaves nothing to ask about; the dialog must not linger
    // describing a process that no longer exists.
    const { view } = setup([process({ id: 4, status: "Running" })]);
    act(() => view.result.current.request(4));
    view.rerender({ list: [] });
    expect(view.result.current.pending).toBeNull();
  });

  it("passes an unknown id straight through for the core to answer for", () => {
    const { close, view } = setup([]);
    act(() => view.result.current.request(99));
    expect(close).toHaveBeenCalledWith(99);
    expect(view.result.current.pending).toBeNull();
  });
});
