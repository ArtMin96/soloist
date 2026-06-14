import { describe, expect, it } from "vitest";
import { applyEvent } from "@/store/projection";
import type { ProcessView } from "@/domain";

const starting: ProcessView = { id: 1, kind: "Command", label: "demo 1", status: "Starting" };

describe("applyEvent", () => {
  it("adds a spawned process", () => {
    const next = applyEvent([], {
      type: "ProcessSpawned",
      id: 1,
      kind: "Command",
      label: "demo 1",
      status: "Starting",
    });
    expect(next).toEqual([starting]);
  });

  it("ignores a duplicate spawn for the same id", () => {
    const next = applyEvent([starting], {
      type: "ProcessSpawned",
      id: 1,
      kind: "Command",
      label: "demo 1",
      status: "Starting",
    });
    expect(next).toHaveLength(1);
  });

  it("updates only the matching process on a status change", () => {
    const other: ProcessView = { ...starting, id: 2 };
    const next = applyEvent([starting, other], {
      type: "ProcessStatusChanged",
      id: 1,
      from: "Starting",
      to: "Running",
    });
    expect(next.find((process) => process.id === 1)?.status).toBe("Running");
    expect(next.find((process) => process.id === 2)).toEqual(other);
  });

  it("removes a process", () => {
    const next = applyEvent([starting], { type: "ProcessRemoved", id: 1 });
    expect(next).toEqual([]);
  });
});
