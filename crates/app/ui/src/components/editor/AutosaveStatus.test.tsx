// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { AUTOSAVE_STATUS_ATTRIBUTE, AutosaveStatus } from "@/components/editor/AutosaveStatus";

afterEach(cleanup);

function status(state: { saving: boolean; dirty: boolean }): HTMLElement {
  render(<AutosaveStatus {...state} />);
  return document.querySelector<HTMLElement>(`[${AUTOSAVE_STATUS_ATTRIBUTE}]`) as HTMLElement;
}

describe("AutosaveStatus", () => {
  // Typing during a write leaves both true, and the write is the thing the reader is waiting on.
  it("reports the write in flight over the keystrokes already held for the next one", () => {
    expect(status({ saving: true, dirty: true }).textContent).toBe("Saving…");
  });

  it("reports edits that have not reached the store yet", () => {
    expect(status({ saving: false, dirty: true }).textContent).toBe("Unsaved changes");
  });

  it("reports saved only with nothing in flight and nothing outstanding", () => {
    expect(status({ saving: false, dirty: false }).textContent).toBe("Saved");
  });

  it("announces politely, so it never interrupts the typing it reports on", () => {
    const label = status({ saving: false, dirty: false });

    expect(label).not.toBeNull();
    expect(label.getAttribute("aria-live")).toBe("polite");
  });
});
