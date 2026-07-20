// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { AdvisoryNotice } from "@/components/AdvisoryNotice";

afterEach(cleanup);

describe("AdvisoryNotice", () => {
  it("interrupts by default, for something that just happened to the user's work", () => {
    const { container } = render(<AdvisoryNotice>The template moved on</AdvisoryNotice>);
    expect(container.querySelector("[role='alert']")).toBeTruthy();
  });

  it("waits for a pause when asked, for an advisory that re-renders as the user types", () => {
    const { container } = render(
      <AdvisoryNotice urgency="status">No value for a placeholder</AdvisoryNotice>,
    );
    expect(container.querySelector("[role='status']")).toBeTruthy();
    expect(container.querySelector("[role='alert']")).toBeNull();
  });

  // The load-bearing one. Politeness is a live accessibility judgement — these strips already moved
  // from `alert` to `status` once, because the preview's advisories re-render per keystroke and were
  // re-interrupting a screen reader on every character. Anything counting strips has to survive that
  // judgement changing again, so a strip carries a marker saying it *is* one, separately from how it
  // asks to be announced. Reading strips by role instead is what silently blinded the preview walk:
  // the role moved, the read matched nothing, and "no notices" is a legitimate state, so it passed.
  it("stays identifiable as an advisory whatever politeness it asks for", () => {
    const { container } = render(
      <>
        <AdvisoryNotice>The template moved on</AdvisoryNotice>
        <AdvisoryNotice urgency="status">No value for a placeholder</AdvisoryNotice>
      </>,
    );
    expect(container.querySelectorAll("[data-advisory-notice]")).toHaveLength(2);
  });

  it("carries the one control that resolves it, when the notice has one", () => {
    render(<AdvisoryNotice action={<button>Reload</button>}>The template moved on</AdvisoryNotice>);
    expect(screen.getByRole("button", { name: "Reload" })).toBeTruthy();
  });
});
