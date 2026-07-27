import { describe, expect, it, vi } from "vitest";
import {
  processActions,
  presentProcessActions,
  runnableProcessActions,
  shouldPersistProcessActions,
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
    expect(
      processActions({
        kind: "Command",
        status: "Stopped",
        requiresTrust: false,
        resumable: false,
      }),
    ).toEqual(["start"]);
  });

  it("offers Resume before Start for a resting resumable agent", () => {
    expect(
      processActions({ kind: "Command", status: "Stopped", requiresTrust: false, resumable: true }),
    ).toEqual(["resume", "start"]);
  });

  it("offers Stop and Restart for a running process, never Start", () => {
    expect(
      processActions({
        kind: "Command",
        status: "Running",
        requiresTrust: false,
        resumable: false,
      }),
    ).toEqual(["stop", "restart"]);
  });

  it.each(["Starting", "Restarting"] as const)(
    "offers only Stop while %s so the in-flight launch can be cancelled",
    (status) => {
      expect(
        processActions({ kind: "Command", status, requiresTrust: false, resumable: false }),
      ).toEqual(["stop"]);
    },
  );

  it("offers only Stop while stopping (no restart of an in-flight stop)", () => {
    expect(
      processActions({
        kind: "Command",
        status: "Stopping",
        requiresTrust: false,
        resumable: false,
      }),
    ).toEqual([]);
  });

  it.each(["Crashed", "RestartExhausted"] as const)(
    "offers Restart rather than Start for %s recovery",
    (status) => {
      expect(
        processActions({ kind: "Command", status, requiresTrust: false, resumable: false }),
      ).toEqual(["restart"]);
    },
  );

  it("offers only Stop when a running process's next launch requires trust", () => {
    expect(
      processActions({ kind: "Command", status: "Running", requiresTrust: true, resumable: false }),
    ).toEqual(["stop"]);
  });

  it("offers no action while stopping even if the next launch requires trust", () => {
    expect(
      processActions({
        kind: "Command",
        status: "Stopping",
        requiresTrust: true,
        resumable: false,
      }),
    ).toEqual([]);
  });

  it("offers only Trust for an untrusted command, withholding Start", () => {
    expect(
      processActions({ kind: "Command", status: "Stopped", requiresTrust: true, resumable: false }),
    ).toEqual(["trust"]);
  });

  it.each(["Agent", "Terminal"] as const)("offers Remove on a resting %s", (kind) => {
    expect(
      processActions({ kind, status: "Stopped", requiresTrust: false, resumable: false }),
    ).toEqual(["start", "remove"]);
  });

  it.each(["Agent", "Terminal"] as const)(
    "offers Remove on a running %s too — the core reaps it before forgetting it",
    (kind) => {
      expect(
        processActions({ kind, status: "Running", requiresTrust: false, resumable: false }),
      ).toEqual(["stop", "restart", "remove"]);
    },
  );

  it("withholds Remove while a terminal is stopping, so no trash icon replaces Stop", () => {
    // Mid-stop it would be the only action, making it the one-click control in the pixel Stop
    // just vacated — a destructive button appearing under a cursor that clicked a safe one.
    expect(
      processActions({
        kind: "Terminal",
        status: "Stopping",
        requiresTrust: false,
        resumable: false,
      }),
    ).toEqual([]);
  });

  it.each(["Stopped", "Running", "Crashed", "RestartExhausted"] as const)(
    "never offers Remove on a %s command — its declaration outlives the process",
    (status) => {
      expect(
        processActions({ kind: "Command", status, requiresTrust: false, resumable: false }),
      ).not.toContain("remove");
    },
  );

  it("orders Remove last, after every run-state action", () => {
    const actions = processActions({
      kind: "Agent",
      status: "Stopped",
      requiresTrust: false,
      resumable: true,
    });
    expect(actions).toEqual(["resume", "start", "remove"]);
  });

  it("withholds Resume from an untrusted resumable agent until it is trusted", () => {
    // The trust gate blocks resume just as it blocks start, so an untrusted process offers only
    // Trust — never an enabled Resume the core would refuse.
    expect(
      processActions({ kind: "Command", status: "Stopped", requiresTrust: true, resumable: true }),
    ).toEqual(["trust"]);
  });
});

describe("presentProcessActions", () => {
  const actions = (kinds: Array<"start" | "stop" | "restart">) => kinds.map((kind) => ({ kind }));

  it("prioritizes Restart for a running command", () => {
    const result = presentProcessActions("Command", "Running", actions(["stop", "restart"]));
    expect(result.primary?.kind).toBe("restart");
    expect(result.secondary.map((action) => action.kind)).toEqual(["stop"]);
  });

  it.each(["Agent", "Terminal"] as const)("prioritizes Stop for a running %s", (kind) => {
    const result = presentProcessActions(kind, "Running", actions(["stop", "restart"]));
    expect(result.primary?.kind).toBe("stop");
    expect(result.secondary.map((action) => action.kind)).toEqual(["restart"]);
  });

  it("keeps Resume ahead of Start for a resumable stopped agent", () => {
    const result = presentProcessActions("Agent", "Stopped", [
      { kind: "resume" as const },
      { kind: "start" as const },
    ]);
    expect(result.primary?.kind).toBe("resume");
    expect(result.secondary.map((action) => action.kind)).toEqual(["start"]);
  });
});

describe("shouldPersistProcessActions", () => {
  it("keeps trust and failed-process recovery visible", () => {
    expect(
      shouldPersistProcessActions({
        kind: "Command",
        status: "Stopped",
        requiresTrust: true,
        resumable: false,
      }),
    ).toBe(true);
    expect(
      shouldPersistProcessActions({
        kind: "Command",
        status: "Crashed",
        requiresTrust: false,
        resumable: false,
      }),
    ).toBe(true);
  });

  it("leaves ordinary and running controls progressively disclosed", () => {
    expect(
      shouldPersistProcessActions({
        kind: "Command",
        status: "Stopped",
        requiresTrust: false,
        resumable: false,
      }),
    ).toBe(false);
    expect(
      shouldPersistProcessActions({
        kind: "Command",
        status: "Running",
        requiresTrust: false,
        resumable: false,
      }),
    ).toBe(false);
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
      onRemove: vi.fn((id) => (calls.remove = [id])),
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

  it("binds Remove to the process's own id", () => {
    const h = handlers();
    const actions = runnableProcessActions(
      process({ id: 12, kind: "Terminal", label: "Terminal 2" }),
      h,
    );
    const remove = actions.find((a) => a.kind === "remove");
    expect(remove?.label).toBe("Remove");
    remove?.run();
    expect(h.calls.remove).toEqual([12]);
  });

  it("names the two resumable-agent choices as continue versus fresh", () => {
    const result = runnableProcessActions(process({ kind: "Agent", resumable: true }), handlers());
    expect(result.map((action) => action.label)).toEqual([
      "Resume last session",
      "Start fresh",
      "Remove",
    ]);
  });
});
