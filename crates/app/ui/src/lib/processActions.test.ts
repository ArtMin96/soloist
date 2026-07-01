import { describe, expect, it, vi } from "vitest";
import {
  processActions,
  runnableProcessActions,
  type ProcessActionHandlers,
} from "@/lib/processActions";
import type { ProcessView } from "@/domain";

function process(overrides: Partial<ProcessView> = {}): ProcessView {
  return {
    id: 7,
    project: 1,
    kind: "Command",
    label: "Web",
    status: "Stopped",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
    ...overrides,
  };
}

describe("processActions", () => {
  it("offers Start (only) for a trusted resting command", () => {
    expect(processActions({ status: "Stopped", requiresTrust: false, resumable: false })).toEqual([
      "start",
    ]);
  });

  it("offers Resume before Start for a resting resumable agent", () => {
    expect(processActions({ status: "Stopped", requiresTrust: false, resumable: true })).toEqual([
      "resume",
      "start",
    ]);
  });

  it("offers Stop and Restart for a running process, never Start", () => {
    expect(processActions({ status: "Running", requiresTrust: false, resumable: false })).toEqual([
      "stop",
      "restart",
    ]);
  });

  it("offers only Stop while stopping (no restart of an in-flight stop)", () => {
    expect(processActions({ status: "Stopping", requiresTrust: false, resumable: false })).toEqual(
      [],
    );
  });

  it("offers only Trust for an untrusted command, withholding Start", () => {
    expect(processActions({ status: "Stopped", requiresTrust: true, resumable: false })).toEqual([
      "trust",
    ]);
  });

  it("withholds Resume from an untrusted resumable agent until it is trusted", () => {
    // The trust gate blocks resume just as it blocks start, so an untrusted process offers only
    // Trust — never an enabled Resume the core would refuse.
    expect(processActions({ status: "Stopped", requiresTrust: true, resumable: true })).toEqual([
      "trust",
    ]);
  });
});

describe("runnableProcessActions", () => {
  function handlers(): ProcessActionHandlers & { calls: Record<string, unknown[]> } {
    const calls: Record<string, unknown[]> = {};
    return {
      calls,
      onTrust: vi.fn((p, n) => (calls.trust = [p, n])),
      onResume: vi.fn((id) => (calls.resume = [id])),
      onStart: vi.fn((id) => (calls.start = [id])),
      onStop: vi.fn((id) => (calls.stop = [id])),
      onRestart: vi.fn((id) => (calls.restart = [id])),
    };
  }

  it("binds each action to its callback with the process's identity", () => {
    const h = handlers();
    const actions = runnableProcessActions(
      process({ id: 42, project: 3, label: "Api", status: "Running" }),
      h,
    );
    actions.find((a) => a.kind === "restart")?.run();
    expect(h.calls.restart).toEqual([42]);
    actions.find((a) => a.kind === "stop")?.run();
    expect(h.calls.stop).toEqual([42]);
  });

  it("trust carries the process's project and label (the gate's key)", () => {
    const h = handlers();
    const actions = runnableProcessActions(
      process({ id: 5, project: 9, label: "Worker", requires_trust: true }),
      h,
    );
    expect(actions.map((a) => a.kind)).toEqual(["trust"]);
    actions[0].run();
    expect(h.calls.trust).toEqual([9, "Worker"]);
  });
});
