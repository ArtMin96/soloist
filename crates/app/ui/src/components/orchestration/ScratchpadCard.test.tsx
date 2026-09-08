// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { CARD_TRIGGER_ATTRIBUTE } from "@/components/common/CardRow";
import {
  SCRATCHPAD_GIST_ATTRIBUTE,
  SCRATCHPAD_TITLE_ATTRIBUTE,
  ScratchpadCard,
} from "@/components/orchestration/ScratchpadCard";
import { SCRATCHPAD_HANDLE_ATTRIBUTE } from "@/components/orchestration/ScratchpadMeta";
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
    gist: "",
    updated_at: NOW - 5 * MINUTE,
    ...overrides,
  };
}

function card(overrides: Partial<Parameters<typeof ScratchpadCard>[0]> = {}) {
  return render(<ScratchpadCard pad={pad()} now={NOW} onOpen={vi.fn()} {...overrides} />);
}

const trigger = () => document.querySelector(`[${CARD_TRIGGER_ATTRIBUTE}]`) as HTMLElement;
const gist = () => document.querySelector(`[${SCRATCHPAD_GIST_ATTRIBUTE}]`);

describe("ScratchpadCard", () => {
  // The row reads as prose; the slug the agent wrote stays reachable beside it rather than being the
  // thing a person has to parse.
  it("reads the row as a title with the handle beside it", () => {
    card({ pad: pad({ name: "release-plan" }) });

    expect(document.querySelector(`[${SCRATCHPAD_TITLE_ATTRIBUTE}]`)?.textContent).toBe(
      "Release plan",
    );
    expect(document.querySelector(`[${SCRATCHPAD_HANDLE_ATTRIBUTE}]`)?.textContent).toBe(
      "release-plan",
    );
  });

  it("separates labeled metadata from the title line with button-safe phrasing content", () => {
    card();

    const title = document.querySelector(`[${SCRATCHPAD_TITLE_ATTRIBUTE}]`) as HTMLElement;
    const metadata = document.querySelector(`[${SCRATCHPAD_HANDLE_ATTRIBUTE}]`)?.parentElement
      ?.parentElement as HTMLElement;
    expect(metadata.previousElementSibling).toBe(title.parentElement);
    expect(title.parentElement?.querySelector("[data-scratchpad-revision]")?.textContent).toBe(
      "Rev 3",
    );
    expect(within(metadata).getByText("Handle")).toBeTruthy();
    expect(within(metadata).getByText("Updated")).toBeTruthy();
    expect(trigger().querySelector("div, dl, dt, dd")).toBeNull();
  });

  it("carries a second line only when the body has a gist to show", () => {
    card({ pad: pad({ gist: "" }) });
    expect(gist()).toBeNull();
    cleanup();

    card({ pad: pad({ gist: "Decisions for the cut" }) });
    expect(gist()?.textContent).toBe("Decisions for the cut");
  });

  it("shows the scratchpad's tags", () => {
    card({ pad: pad({ tags: ["infra", "release"] }) });

    expect(within(trigger()).getByText("infra")).toBeTruthy();
    expect(within(trigger()).getByText("release")).toBeTruthy();
  });

  // The whole face of the card is one control, so nothing inside it may be another: a nested control
  // is unreachable by keyboard and swallows the click meant for the row.
  it("buries no control inside the row's own trigger", () => {
    card({ pad: pad({ gist: "Decisions for the cut", tags: ["infra"], archived: true }) });

    expect(trigger().querySelectorAll("button, a, input, [tabindex]").length).toBe(0);
  });

  it("hands the pane over when the row is chosen", () => {
    const onOpen = vi.fn();
    card({ onOpen });

    fireEvent.click(screen.getByRole("button"));

    expect(onOpen).toHaveBeenCalledTimes(1);
  });
});
