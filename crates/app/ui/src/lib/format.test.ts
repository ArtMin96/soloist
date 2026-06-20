import { describe, expect, it } from "vitest";
import { formatCpu, formatPorts, formatRss } from "@/lib/format";

describe("formatCpu", () => {
  it("rounds to a whole percent", () => {
    expect(formatCpu(0)).toBe("0%");
    expect(formatCpu(3.6)).toBe("4%");
  });

  it("keeps multi-core values above 100%", () => {
    expect(formatCpu(240.2)).toBe("240%");
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
