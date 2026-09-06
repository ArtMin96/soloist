import { describe, expect, it } from "vitest";
import { formatCpu, formatPorts, formatRss, formatUpdatedAt } from "@/lib/format";

describe("formatCpu", () => {
  it("rounds to a whole percent", () => {
    expect(formatCpu(0)).toBe("0%");
    expect(formatCpu(3.6)).toBe("4%");
  });

  it("rounds a near-saturated reading up to 100%", () => {
    expect(formatCpu(99.6)).toBe("100%");
  });
});

describe("formatRss", () => {
  it("shows small sizes in KB", () => {
    expect(formatRss(512 * 1024)).toBe("512 KB");
  });

  it("shows megabyte sizes as whole MB", () => {
    expect(formatRss(86 * 1024 * 1024)).toBe("86 MB");
  });

  it("shows gigabyte sizes with one decimal", () => {
    expect(formatRss(1.5 * 1024 * 1024 * 1024)).toBe("1.5 GB");
  });
});

describe("formatPorts", () => {
  it("is null when nothing is listening", () => {
    expect(formatPorts([])).toBeNull();
  });

  it("shows a single port", () => {
    expect(formatPorts([5173])).toBe(":5173");
  });

  it("shows the first port and an overflow count", () => {
    expect(formatPorts([5173, 9229, 24678])).toBe(":5173 +2");
  });
});

describe("formatUpdatedAt", () => {
  // Midday, so the calendar day the formatter prints cannot shift with the runner's time zone.
  const now = new Date(2026, 8, 4, 12).getTime();
  const SECOND = 1000;
  const MINUTE = 60 * SECOND;
  const HOUR = 60 * MINUTE;
  const DAY = 24 * HOUR;

  it("is nothing for a document written before write times were recorded", () => {
    expect(formatUpdatedAt(0, now)).toBeNull();
  });

  it("reads a write from seconds ago as just now", () => {
    expect(formatUpdatedAt(now - 10 * SECOND, now)).toBe("just now");
  });

  it("counts minutes, then hours, then days", () => {
    expect(formatUpdatedAt(now - 5 * MINUTE, now)).toBe("5 min ago");
    expect(formatUpdatedAt(now - 3 * HOUR, now)).toBe("3 h ago");
    expect(formatUpdatedAt(now - 2 * DAY, now)).toBe("2 d ago");
  });

  it("gives the calendar date once a write is a week or more old", () => {
    const formatted = formatUpdatedAt(now - 30 * DAY, now);
    expect(formatted).not.toContain("ago");
    expect(formatted).toContain("5");
  });

  it("names the year of a write from an earlier year", () => {
    expect(formatUpdatedAt(new Date(2025, 10, 20, 12).getTime(), now)).toContain("2025");
  });
});
