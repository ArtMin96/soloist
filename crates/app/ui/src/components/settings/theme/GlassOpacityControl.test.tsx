// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ThemeSettings } from "@/components/settings/theme/ThemeSettings";
import type { Appearance } from "@/domain";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { AppearanceProvider } from "@/store/AppearanceProvider";
import { GLASS_OPACITY } from "@/theme/constraints";

// A held arrow key steps the slider once per repeat. Each step is a value the user passes through,
// not one they chose, so only the first and the one they land on need to reach the core.
const WRITES_PER_SWEEP = 2;

const api = vi.hoisted(() => ({
  read: vi.fn<() => Promise<Appearance>>(),
  write: vi.fn(),
  selectTheme: vi.fn(),
  createTheme: vi.fn(),
  updateTheme: vi.fn(),
  importTheme: vi.fn(),
  inspectTheme: vi.fn(),
  duplicateTheme: vi.fn(),
  removeTheme: vi.fn(),
  setGlassOpacity: vi.fn<(opacity: number) => Promise<Appearance>>(),
}));

vi.mock("@/api", () => ({
  appearance: api.read,
  setAppearance: api.write,
  selectTheme: api.selectTheme,
  createTheme: api.createTheme,
  updateTheme: api.updateTheme,
  importTheme: api.importTheme,
  inspectTheme: api.inspectTheme,
  duplicateTheme: api.duplicateTheme,
  removeTheme: api.removeTheme,
  setGlassOpacity: api.setGlassOpacity,
}));

vi.mock("@/lib/clipboard", () => ({ writeClipboard: vi.fn(() => Promise.resolve()) }));

function renderPanel() {
  render(
    <AppearanceProvider>
      <ThemeSettings />
    </AppearanceProvider>,
  );
  return screen.findByRole("slider", { name: "Glass opacity" });
}

describe("Glass opacity control", () => {
  afterEach(cleanup);
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    document.documentElement.removeAttribute("style");
    const stored: Appearance = { ...DEFAULT_APPEARANCE, glass_opacity: GLASS_OPACITY.min };
    api.read.mockResolvedValue(stored);
    api.setGlassOpacity.mockImplementation(async (opacity) => {
      stored.glass_opacity = opacity;
      return structuredClone(stored);
    });
  });

  it("steps the whole range without waiting on the write for each step", async () => {
    const slider = await renderPanel();
    await waitFor(() => expect(slider.getAttribute("aria-valuenow")).toBe(`${GLASS_OPACITY.min}`));

    const steps = (GLASS_OPACITY.max - GLASS_OPACITY.min) / GLASS_OPACITY.step;
    for (let index = 0; index < steps; index += 1) {
      fireEvent.keyDown(slider, { key: "ArrowRight" });
    }

    expect(slider.getAttribute("aria-valuenow")).toBe(`${GLASS_OPACITY.max}`);
    expect(screen.getByText(`${GLASS_OPACITY.max}%`)).toBeTruthy();
    await waitFor(() =>
      expect(document.documentElement.style.getPropertyValue("--glass-opacity")).toBe("1"),
    );
    expect(api.setGlassOpacity.mock.calls.length).toBeLessThanOrEqual(WRITES_PER_SWEEP);
    expect(api.setGlassOpacity).toHaveBeenLastCalledWith(GLASS_OPACITY.max);
  });

  it("follows an opacity chosen elsewhere once it is persisted", async () => {
    const slider = await renderPanel();
    await waitFor(() => expect(slider.getAttribute("aria-valuenow")).toBe(`${GLASS_OPACITY.min}`));

    fireEvent.keyDown(slider, { key: "ArrowRight" });
    await waitFor(() =>
      expect(slider.getAttribute("aria-valuenow")).toBe(
        `${GLASS_OPACITY.min + GLASS_OPACITY.step}`,
      ),
    );

    fireEvent.click(screen.getByRole("button", { name: "Reset glass opacity" }));

    await waitFor(() =>
      expect(slider.getAttribute("aria-valuenow")).toBe(`${GLASS_OPACITY.default}`),
    );
    expect(screen.getByText(`${GLASS_OPACITY.default}%`)).toBeTruthy();
  });
});
