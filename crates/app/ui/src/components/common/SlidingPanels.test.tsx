// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { SlidingPanels, type SlidingPanel } from "@/components/common/SlidingPanels";

afterEach(cleanup);

function renderPanels(showing: SlidingPanel, onSettled = () => {}) {
  return render(
    <SlidingPanels
      showing={showing}
      list={<button type="button">In list</button>}
      detail={<button type="button">In detail</button>}
      onSettled={onSettled}
    />,
  );
}

const track = (container: HTMLElement) => container.querySelector("[data-panel-route] > div")!;
const panel = (container: HTMLElement, name: SlidingPanel) =>
  container.querySelector(`[data-panel="${name}"]`)!;

describe("SlidingPanels", () => {
  it("holds the off-screen panel inert, and flips it with the route", () => {
    // jsdom implements the `inert` attribute but none of its behaviour, so this checks the
    // attribute reaches the DOM. That it actually removes the panel from the tab order and the
    // accessibility tree is only observable in a real window.
    const { container, rerender } = renderPanels("list");
    expect(panel(container, "list").hasAttribute("inert")).toBe(false);
    expect(panel(container, "detail").hasAttribute("inert")).toBe(true);

    rerender(<SlidingPanels showing="detail" list={null} detail={null} onSettled={() => {}} />);
    expect(panel(container, "list").hasAttribute("inert")).toBe(true);
    expect(panel(container, "detail").hasAttribute("inert")).toBe(false);
  });

  // The property names below are the ones a real window emits. Measured on the track in WebKitGTK:
  // `["start translate self=true", "start box-shadow self=false", …]`, with computed
  // `transitionProperty: "transform, translate, scale, rotate"` and computed `transform: "none"`.
  // Tailwind compiles `-translate-x-full` to `translate`, so `translate` is what actually arrives —
  // a test that fires `transform` describes a code path the browser can never take.
  it("settles on the translate the browser actually emits for the track", () => {
    const onSettled = vi.fn();
    const { container } = renderPanels("detail", onSettled);
    fireEvent.transitionEnd(track(container), { propertyName: "translate" });
    expect(onSettled).toHaveBeenCalledTimes(1);
  });

  it("settles on transform too, for a build that compiles the class that way", () => {
    const onSettled = vi.fn();
    const { container } = renderPanels("detail", onSettled);
    fireEvent.transitionEnd(track(container), { propertyName: "transform" });
    expect(onSettled).toHaveBeenCalledTimes(1);
  });

  it("ignores a transition that bubbled up from inside a panel", () => {
    const onSettled = vi.fn();
    const { container } = renderPanels("detail", onSettled);
    // Content inside a panel animates too; its movement must not be read as the track arriving.
    fireEvent.transitionEnd(panel(container, "detail"), { propertyName: "translate" });
    expect(onSettled).not.toHaveBeenCalled();
  });

  it("ignores a property that is not the movement, on the track itself", () => {
    const onSettled = vi.fn();
    const { container } = renderPanels("detail", onSettled);
    // `box-shadow` is in the measured stream rather than an invented name.
    fireEvent.transitionEnd(track(container), { propertyName: "box-shadow" });
    expect(onSettled).not.toHaveBeenCalled();
  });

  it("names the route it is showing", () => {
    const { container } = renderPanels("list");
    expect(container.querySelector("[data-panel-route]")!.getAttribute("data-panel-route")).toBe(
      "list",
    );
  });
});
