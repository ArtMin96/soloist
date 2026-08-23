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
    total: entries.reduce((sum, entry) => sum + entry.alerts, 0),
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
    const unread = unreadProcessIds(snapshot({ process: 2, kind: "crashed", alerts: 1 }));
    expect(unread.has(2)).toBe(true);
    expect(unread.has(1)).toBe(false);
  });

  it("names nothing when nothing is unread", () => {
    expect(unreadProcessIds(snapshot()).size).toBe(0);
  });
});

describe("unreadProjectIds", () => {
  it("names the project owning an unread process", () => {
    const projects = unreadProjectIds(snapshot({ process: 2, kind: "crashed", alerts: 1 }), STACK);
    expect(projects.has(10)).toBe(true);
    expect(projects.has(20)).toBe(false);
  });

  it("names a project once however many of its processes are unread", () => {
    const projects = unreadProjectIds(
      snapshot(
        { process: 1, kind: "crashed", alerts: 1 },
        { process: 2, kind: "agent_error", alerts: 1 },
      ),
      STACK,
    );
    expect([...projects]).toEqual([10]);
  });

  it("ignores an unread process that is no longer in the stack", () => {
    const projects = unreadProjectIds(snapshot({ process: 99, kind: "crashed", alerts: 1 }), STACK);
    expect(projects.size).toBe(0);
  });
});

describe("attentionEntries", () => {
  it("names each unread process and the oldest kind waiting on it", () => {
    const entries = attentionEntries(
      snapshot({ process: 2, kind: "terminal_bell", alerts: 2 }),
      STACK,
    );
    expect(entries).toEqual([{ process: 2, label: "api", kind: "terminal_bell", alerts: 2 }]);
  });

  it("reads the same for one alert, a few, and far past the display cap", () => {
    const one = snapshot({ process: 1, kind: "crashed", alerts: 1 });
    const few = snapshot({ process: 1, kind: "crashed", alerts: 2 });
    const many = snapshot({ process: 1, kind: "crashed", alerts: 150 });

    // The kind never changes with how much has piled up, each entry carries the core's own count,
    // and only the title bar's reading caps.
    expect(attentionEntries(one, STACK)).toEqual([
      { process: 1, label: "web", kind: "crashed", alerts: 1 },
    ]);
    expect(attentionEntries(few, STACK)).toEqual([
      { process: 1, label: "web", kind: "crashed", alerts: 2 },
    ]);
    expect(attentionEntries(many, STACK)).toEqual([
      { process: 1, label: "web", kind: "crashed", alerts: 150 },
    ]);
    expect([one, few, many].map((each) => attentionCountLabel(each.total))).toEqual([
      "1",
      "2",
      "99+",
    ]);
  });

  it("keeps the core's order", () => {
    const entries = attentionEntries(
      snapshot(
        { process: 3, kind: "crashed", alerts: 1 },
        { process: 1, kind: "agent_error", alerts: 1 },
      ),
      STACK,
    );
    expect(entries.map((entry) => entry.label)).toEqual(["docs", "web"]);
  });

  it("drops a process the stack no longer holds, so a removed row leaves no entry", () => {
    const entries = attentionEntries(
      snapshot(
        { process: 99, kind: "crashed", alerts: 1 },
        { process: 1, kind: "crashed", alerts: 1 },
      ),
      STACK,
    );
    expect(entries.map((entry) => entry.process)).toEqual([1]);
  });

  it("lists nothing when nothing is unread", () => {
    expect(attentionEntries(snapshot(), STACK)).toEqual([]);
  });
});
