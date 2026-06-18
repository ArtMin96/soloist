import { describe, expect, it } from "vitest";
import { applyEvent } from "@/store/projection";
import type { ProcessView } from "@/domain";

const starting: ProcessView = {
  id: 1,
  project: 1,
  kind: "Command",
  label: "web",
  status: "Starting",
  exit_code: null,
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
    });
    expect(next).toEqual([starting]);
  });

  it("ignores a duplicate spawn for the same id", () => {
    const next = applyEvent([starting], {
      type: "ProcessSpawned",
      id: 1,
      project: 1,
      kind: "Command",
      label: "web",
      status: "Starting",
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

  it("removes a process", () => {
    const next = applyEvent([starting], { type: "ProcessRemoved", id: 1 });
    expect(next).toEqual([]);
  });

  it("leaves the process list untouched for non-process events", () => {
    expect(applyEvent([starting], { type: "TerminalBell", id: 1 })).toEqual([starting]);
  });
});
