// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { AppearancePanel } from "@/components/settings/AppearancePanel";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { AppearanceContext } from "@/store/appearanceContext";
import { fakeAppearanceState } from "@/test/appearanceState";
import type { Appearance } from "@/domain";

afterEach(cleanup);

function renderPanel(font_family: string | null) {
  const appearance: Appearance = {
    ...DEFAULT_APPEARANCE,
    terminal: { ...DEFAULT_APPEARANCE.terminal, font_family },
  };
  return render(
    <AppearanceContext value={fakeAppearanceState(appearance, false)}>
      <AppearancePanel />
    </AppearanceContext>,
  );
}

const fontFamilyControl = () => screen.getByRole("combobox", { name: "Font family" });

describe("Settings — terminal font family", () => {
  it("shows the stored family even when it is not one the picker offers", () => {
    // A family chosen before the offered set was narrowed is still what the record holds. The
    // control reads the record, so if it carries no matching item the user is shown nothing at
    // all — their setting reads as unset while the terminal keeps rendering it.
    renderPanel("Fira Code");
    expect(fontFamilyControl().textContent).toBe("Fira Code");
  });

  it("names the system default when no family is stored", () => {
    renderPanel(null);
    expect(fontFamilyControl().textContent).toBe("System default");
  });

  it("still renders the panel when the stored family is blank", () => {
    // The field is a free string, so a hand-written record can hold "". Offering it as an item
    // would give the select an empty value, which Radix refuses by throwing — taking the whole
    // settings panel down rather than the one row that could not be shown.
    renderPanel("");
    expect(fontFamilyControl()).toBeTruthy();
  });
});
