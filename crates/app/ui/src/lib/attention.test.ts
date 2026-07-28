import { describe, expect, it } from "vitest";
import {
  ATTENTION_DISPLAY_CAP,
  attentionCountLabel,
  attentionEntries,
  unreadProcessIds,
  unreadProjectIds,
} from "@/lib/attention";
import type { AttentionSnapshot, ProcessView } from "@/domain";

function process(id: number, project: number, label: string): ProcessView {
  return {
    id,
    project,
    kind: "Command",
    label,
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
  };
}

const STACK = [process(1, 10, "web"), process(2, 10, "api"), process(3, 20, "docs")];

function snapshot(...entries: AttentionSnapshot["processes"]): AttentionSnapshot {
  return {
    processes: entries,
    total: entries.reduce((sum, entry) => sum + entry.kinds.length, 0),
  };
}

describe("attentionCountLabel", () => {
  it("reads the exact total up to the cap", () => {
    expect(attentionCountLabel(1)).toBe("1");
    expect(attentionCountLabel(ATTENTION_DISPLAY_CAP)).toBe("99");
  });

  it("reads 99+ past the cap, however far past", () => {
    expect(attentionCountLabel(ATTENTION_DISPLAY_CAP + 1)).toBe("99+");
    expect(attentionCountLabel(150)).toBe("99+");
    expect(attentionCountLabel(10_000)).toBe("99+");
  });
});

describe("unreadProcessIds", () => {
  it("names every process with something waiting", () => {
    const unread = unreadProcessIds(snapshot({ process: 2, kinds: ["crashed"] }));
    expect(unread.has(2)).toBe(true);
    expect(unread.has(1)).toBe(false);
  });

  it("names nothing when nothing is unread", () => {
    expect(unreadProcessIds(snapshot()).size).toBe(0);
  });
});

describe("unreadProjectIds", () => {
  it("names the project owning an unread process", () => {
    const projects = unreadProjectIds(snapshot({ process: 2, kinds: ["crashed"] }), STACK);
    expect(projects.has(10)).toBe(true);
    expect(projects.has(20)).toBe(false);
  });

  it("names a project once however many of its processes are unread", () => {
    const projects = unreadProjectIds(
      snapshot({ process: 1, kinds: ["crashed"] }, { process: 2, kinds: ["agent_error"] }),
      STACK,
    );
    expect([...projects]).toEqual([10]);
  });

  it("ignores an unread process that is no longer in the stack", () => {
    const projects = unreadProjectIds(snapshot({ process: 99, kinds: ["crashed"] }), STACK);
    expect(projects.size).toBe(0);
  });
});

describe("attentionEntries", () => {
  it("names each unread process and the oldest kind waiting on it", () => {
    const entries = attentionEntries(
      snapshot({ process: 2, kinds: ["terminal_bell", "crashed"] }),
      STACK,
    );
    expect(entries).toEqual([{ process: 2, label: "api", kind: "terminal_bell", alerts: 2 }]);
  });

  it("keeps the core's order", () => {
    const entries = attentionEntries(
      snapshot({ process: 3, kinds: ["crashed"] }, { process: 1, kinds: ["agent_error"] }),
      STACK,
    );
    expect(entries.map((entry) => entry.label)).toEqual(["docs", "web"]);
  });

  it("drops a process the stack no longer holds, so a removed row leaves no entry", () => {
    const entries = attentionEntries(
      snapshot({ process: 99, kinds: ["crashed"] }, { process: 1, kinds: ["crashed"] }),
      STACK,
    );
    expect(entries.map((entry) => entry.process)).toEqual([1]);
  });

  it("lists nothing when nothing is unread", () => {
    expect(attentionEntries(snapshot(), STACK)).toEqual([]);
  });
});
