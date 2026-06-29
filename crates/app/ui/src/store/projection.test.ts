import { describe, expect, it } from "vitest";
import { applyEvent } from "@/store/projection";
import type { DomainEvent, ProcessView } from "@/domain";

const starting: ProcessView = {
  id: 1,
  project: 1,
  kind: "Command",
  label: "web",
  status: "Starting",
  exit_code: null,
  requires_trust: false,
  resumable: false,
  ports: [],
  ready: "Ungated",
};

describe("applyEvent", () => {
  it("adds a spawned process with its project and a cleared exit code", () => {
    const next = applyEvent([], {
      type: "ProcessSpawned",
      id: 1,
      project: 1,
      kind: "Command",
      label: "web",
      status: "Starting",
      requires_trust: false,
      resumable: false,
    });
    expect(next).toEqual([starting]);
  });

  it("carries the spawned command's trust state", () => {
    const next = applyEvent([], {
      type: "ProcessSpawned",
      id: 2,
      project: 1,
      kind: "Command",
      label: "api",
      status: "Stopped",
      requires_trust: true,
      resumable: false,
    });
    expect(next[0]?.requires_trust).toBe(true);
  });

  it("carries a spawned agent's resumable flag", () => {
    const next = applyEvent([], {
      type: "ProcessSpawned",
      id: 3,
      project: 1,
      kind: "Agent",
      label: "assistant",
      status: "Stopped",
      requires_trust: false,
      resumable: true,
    });
    expect(next[0]?.resumable).toBe(true);
  });

  it("ignores a duplicate spawn for the same id", () => {
    const next = applyEvent([starting], {
      type: "ProcessSpawned",
      id: 1,
      project: 1,
      kind: "Command",
      label: "web",
      status: "Starting",
      requires_trust: false,
      resumable: false,
    });
    expect(next).toHaveLength(1);
  });

  it("updates status and exit code only on the matching process", () => {
    const other: ProcessView = { ...starting, id: 2 };
    const next = applyEvent([starting, other], {
      type: "ProcessStatusChanged",
      id: 1,
      from: "Running",
      to: "Crashed",
      exit_code: 3,
    });
    const changed = next.find((process) => process.id === 1);
    expect(changed?.status).toBe("Crashed");
    expect(changed?.exit_code).toBe(3);
    expect(next.find((process) => process.id === 2)).toEqual(other);
  });

  it("updates the listening ports only on the matching process", () => {
    const other: ProcessView = { ...starting, id: 2 };
    const next = applyEvent([starting, other], {
      type: "PortsChanged",
      id: 1,
      ports: [5173, 8080],
    });
    expect(next.find((process) => process.id === 1)?.ports).toEqual([5173, 8080]);
    expect(next.find((process) => process.id === 2)?.ports).toEqual([]);
  });

  it("maps the readiness event to the gate only on the matching process", () => {
    const other: ProcessView = { ...starting, id: 2 };
    const waiting = applyEvent([starting, other], {
      type: "ReadyStateChanged",
      id: 1,
      ready: false,
    });
    expect(waiting.find((process) => process.id === 1)?.ready).toBe("Waiting");
    expect(waiting.find((process) => process.id === 2)?.ready).toBe("Ungated");

    const ready = applyEvent(waiting, { type: "ReadyStateChanged", id: 1, ready: true });
    expect(ready.find((process) => process.id === 1)?.ready).toBe("Ready");
  });

  it("removes a process", () => {
    const next = applyEvent([starting], { type: "ProcessRemoved", id: 1 });
    expect(next).toEqual([]);
  });

  it("renames only the matching process", () => {
    const other: ProcessView = { ...starting, id: 2 };
    const next = applyEvent([starting, other], {
      type: "ProcessRenamed",
      id: 1,
      label: "renamed",
    });
    expect(next.find((process) => process.id === 1)?.label).toBe("renamed");
    expect(next.find((process) => process.id === 2)?.label).toBe("web");
  });

  it("leaves the process list untouched for non-process events", () => {
    expect(applyEvent([starting], { type: "TerminalBell", id: 1 })).toEqual([starting]);
    expect(applyEvent([starting], { type: "ProjectOpened", id: 1 })).toEqual([starting]);
    expect(applyEvent([starting], { type: "MetricsTick", id: 1, cpu_pct: 12, rss: 4096 })).toEqual([
      starting,
    ]);
  });

  it("leaves the process list untouched for coordination events (the orchestration snapshot owns them)", () => {
    const coordination: DomainEvent[] = [
      { type: "TodoChanged", project: 1, id: 7 },
      { type: "TimerArmed", owner: 1, id: 2 },
      { type: "TimerFired", owner: 1, id: 2 },
      { type: "TimerCleared", owner: 1, id: 2 },
      { type: "LeaseChanged", project: 1, key: "deploy" },
      { type: "ScratchpadChanged", project: 1, name: "plan" },
      { type: "KvChanged", project: 1, key: "config" },
    ];
    const input = [starting];
    for (const event of coordination) {
      // Referential identity: the reducer returns the same array, never re-allocating for a
      // delta that does not touch the process list.
      expect(applyEvent(input, event)).toBe(input);
    }
  });
});
