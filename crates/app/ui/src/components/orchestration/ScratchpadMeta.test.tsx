// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import {
  SCRATCHPAD_ARCHIVED_ATTRIBUTE,
  SCRATCHPAD_HANDLE_ATTRIBUTE,
  SCRATCHPAD_REVISION_ATTRIBUTE,
  SCRATCHPAD_UPDATED_ATTRIBUTE,
  ScratchpadMeta,
  ScratchpadRevision,
} from "@/components/orchestration/ScratchpadMeta";
import type { ScratchpadSummary } from "@/domain";

afterEach(cleanup);

const NOW = new Date(2026, 0, 20, 12).getTime();
const MINUTE = 60_000;

function pad(overrides: Partial<ScratchpadSummary> = {}): ScratchpadSummary {
  return {
    id: 4,
    name: "release-plan",
    tags: [],
    archived: false,
    revision: 3,
    gist: "the plan",
    updated_at: NOW - 5 * MINUTE,
    ...overrides,
  };
}

const rail = (overrides: Partial<ScratchpadSummary> = {}) =>
  render(<ScratchpadMeta pad={pad(overrides)} now={NOW} />);

const handle = () => document.querySelector(`[${SCRATCHPAD_HANDLE_ATTRIBUTE}]`);
const updated = () => document.querySelector(`[${SCRATCHPAD_UPDATED_ATTRIBUTE}]`);

describe("ScratchpadMeta", () => {
  it("always labels the raw handle, including when it reads like the title", () => {
    rail({ name: "research" });
    expect(screen.getByText("Handle")).toBeTruthy();
    expect(handle()?.textContent).toBe("research");
    cleanup();

    rail({ name: "release-plan" });
    expect(handle()?.textContent).toBe("release-plan");
  });

  it("keeps the revision in a compact mono token", () => {
    render(<ScratchpadRevision revision={12} />);

    const revision = document.querySelector(`[${SCRATCHPAD_REVISION_ATTRIBUTE}]`) as HTMLElement;
    expect(revision.textContent).toBe("Rev 12");
    expect(revision.className).toContain("font-mono");
    expect(revision.className).toContain("tabular-nums");
  });

  it("names how long ago the body was written", () => {
    rail({ updated_at: NOW - 5 * MINUTE });

    expect(screen.getByText("Updated")).toBeTruthy();
    expect(updated()?.textContent).toBe("5 min ago");
  });

  // A scratchpad written before the core recorded write times carries no stamp at all. Rendering the
  // element anyway would date every such document to 1970 — an absent time is not a write at the epoch.
  it("says when a document carries no recorded write time without inventing a date", () => {
    rail({ updated_at: 0 });

    expect(updated()?.textContent).toBe("Not recorded");
    expect(document.querySelector("time")).toBeNull();
    expect(screen.queryByText(/1970/)).toBeNull();
  });

  it("uses compact values on cards and roomier values in detail", () => {
    const { rerender } = render(<ScratchpadMeta pad={pad()} now={NOW} variant="card" />);
    expect(handle()?.className).toContain("type-label");

    rerender(<ScratchpadMeta pad={pad()} now={NOW} variant="detail" />);
    expect(handle()?.className).toContain("type-body");
  });

  it("marks an archived scratchpad and leaves an active one unmarked", () => {
    rail({ archived: false });
    expect(document.querySelector(`[${SCRATCHPAD_ARCHIVED_ATTRIBUTE}]`)).toBeNull();
    cleanup();

    rail({ archived: true });
    expect(document.querySelector(`[${SCRATCHPAD_ARCHIVED_ATTRIBUTE}]`)?.textContent).toBe(
      "Archived",
    );
  });
});
