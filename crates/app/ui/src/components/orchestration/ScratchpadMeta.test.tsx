// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import {
  SCRATCHPAD_ARCHIVED_ATTRIBUTE,
  SCRATCHPAD_HANDLE_ATTRIBUTE,
  SCRATCHPAD_REVISION_ATTRIBUTE,
  SCRATCHPAD_UPDATED_ATTRIBUTE,
  ScratchpadMeta,
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
  // A name the user wrote is its own handle; printing it twice beside a title that reads identically
  // is noise, so the handle earns its place only when humanizing actually changed something.
  it("shows the handle only when the title no longer reads as it", () => {
    rail({ name: "research" });
    expect(handle()).toBeNull();
    cleanup();

    rail({ name: "release-plan" });
    expect(handle()?.textContent).toBe("release-plan");
  });

  it("sets the revision in mono so digits align between rows", () => {
    rail({ revision: 12 });

    const revision = document.querySelector(`[${SCRATCHPAD_REVISION_ATTRIBUTE}]`) as HTMLElement;
    expect(revision.textContent).toBe("r12");
    expect(revision.className).toContain("font-mono");
    expect(revision.className).toContain("tabular-nums");
  });

  it("names how long ago the body was written", () => {
    rail({ updated_at: NOW - 5 * MINUTE });

    expect(updated()?.textContent).toBe("5 min ago");
  });

  // A scratchpad written before the core recorded write times carries no stamp at all. Rendering the
  // element anyway would date every such document to 1970 — an absent time is not a write at the epoch.
  it("renders no time at all for a document that carries no write time", () => {
    rail({ updated_at: 0 });

    expect(updated()).toBeNull();
    expect(document.querySelector("time")).toBeNull();
    expect(screen.queryByText(/1970/)).toBeNull();
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
