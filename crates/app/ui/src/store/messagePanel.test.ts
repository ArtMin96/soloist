import { describe, expect, it } from "vitest";
import {
  agentLabels,
  isAtRetentionCeiling,
  MAX_RETAINED_MESSAGES,
  VISIBLE_MESSAGE_WINDOW,
  windowMessages,
} from "@/store/messagePanel";
import type { AgentMessageRecord, AgentNode } from "@/domain";

function record(id: number): AgentMessageRecord {
  return {
    delivery: {
      message: {
        id,
        project: 1,
        sender: 1,
        recipient: 2,
        kind: "direct",
        body: `body-${id}`,
        todo_id: null,
      },
      outcome: "queued",
    },
    at_unix_millis: 1_700_000_000_000 + id,
    truncated: false,
  };
}

function records(count: number): AgentMessageRecord[] {
  return Array.from({ length: count }, (_, index) => record(index));
}

function agent(id: number, label: string): AgentNode {
  return { id, parent: null, label, kind: "Agent", status: "Running", activity: null };
}

describe("agentLabels", () => {
  it("resolves a process id to the label the tree shows", () => {
    const labelOf = agentLabels([agent(1, "lead"), agent(2, "worker 2")]);
    expect(labelOf(1)).toBe("lead");
    expect(labelOf(2)).toBe("worker 2");
  });

  it("falls back to the id for an agent that has left the registry", () => {
    const labelOf = agentLabels([agent(1, "lead")]);
    expect(labelOf(9)).toBe("#9");
  });
});

describe("isAtRetentionCeiling", () => {
  it("is quiet while the whole conversation still fits", () => {
    expect(isAtRetentionCeiling(records(3))).toBe(false);
    expect(isAtRetentionCeiling(records(MAX_RETAINED_MESSAGES - 1))).toBe(false);
  });

  it("reports the ceiling once the retained log is full", () => {
    expect(isAtRetentionCeiling(records(MAX_RETAINED_MESSAGES))).toBe(true);
  });
});

describe("windowMessages", () => {
  it("shows everything while the log is shorter than the window", () => {
    const all = records(VISIBLE_MESSAGE_WINDOW);
    expect(windowMessages(all, false)).toEqual({ visible: all, hidden: 0 });
  });

  it("keeps the most recent exchanges and counts the ones held back", () => {
    const all = records(VISIBLE_MESSAGE_WINDOW + 5);
    const { visible, hidden } = windowMessages(all, false);
    expect(hidden).toBe(5);
    expect(visible).toHaveLength(VISIBLE_MESSAGE_WINDOW);
    expect(visible[visible.length - 1]).toBe(all[all.length - 1]);
    expect(visible[0]).toBe(all[5]);
  });

  it("shows the whole retained log once expanded", () => {
    const all = records(VISIBLE_MESSAGE_WINDOW + 5);
    expect(windowMessages(all, true)).toEqual({ visible: all, hidden: 0 });
  });
});
