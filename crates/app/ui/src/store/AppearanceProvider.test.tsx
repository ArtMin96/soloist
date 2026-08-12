// @vitest-environment jsdom

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Appearance, ThemeAppearance, ThemeConflictPolicy, ThemeFile } from "@/domain";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { AppearanceProvider } from "@/store/AppearanceProvider";
import { useAppearance } from "@/store/appearanceContext";
import { BUILT_IN_THEMES } from "@/theme/catalog";

const api = vi.hoisted(() => ({
  read: vi.fn<() => Promise<Appearance>>(),
  write: vi.fn<(appearance: Appearance) => Promise<Appearance>>(),
  selectTheme: vi.fn<(appearance: ThemeAppearance, themeId: string) => Promise<Appearance>>(),
  createTheme: vi.fn<(theme: ThemeFile) => Promise<Appearance>>(),
  updateTheme: vi.fn<(theme: ThemeFile) => Promise<Appearance>>(),
  importTheme: vi.fn<(json: string, conflict: ThemeConflictPolicy) => Promise<Appearance>>(),
  inspectTheme: vi.fn<(json: string) => Promise<ThemeFile>>(),
  duplicateTheme: vi.fn<(themeId: string) => Promise<Appearance>>(),
  removeTheme: vi.fn<(themeId: string) => Promise<Appearance>>(),
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

function Probe() {
  const { appliedTheme, selectTheme, setGlassOpacity } = useAppearance();
  return (
    <>
      <output aria-label="theme">{appliedTheme.id}</output>
      <button type="button" onClick={() => void selectTheme("dracula", "dark")}>
        Use Dracula
      </button>
      <button type="button" onClick={() => void setGlassOpacity(65)}>
        Set glass
      </button>
    </>
  );
}

function CoreMutationProbe() {
  const { customThemes, duplicateTheme } = useAppearance();
  return (
    <>
      <output aria-label="custom-themes">{customThemes.map(({ id }) => id).join(",")}</output>
      <button type="button" onClick={() => void duplicateTheme("dracula")}>
        Duplicate Dracula
      </button>
    </>
  );
}

describe("AppearanceProvider theme runtime", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    document.documentElement.removeAttribute("style");
    document.documentElement.className = "";
    const stored: Appearance = {
      ...DEFAULT_APPEARANCE,
      theme: "dark",
      selected_themes: { light: "soloist-default", dark: "poimandres-dark-theme" },
    };
    api.read.mockResolvedValue(stored);
    api.write.mockImplementation(async (appearance) => appearance);
    api.selectTheme.mockImplementation(async (themeAppearance, themeId) => {
      stored.selected_themes = { ...stored.selected_themes, [themeAppearance]: themeId };
      return structuredClone(stored);
    });
    api.setGlassOpacity.mockImplementation(async (opacity) => {
      stored.glass_opacity = opacity;
      return structuredClone(stored);
    });
  });

  it("applies and persists same-appearance theme and glass changes through one context", async () => {
    render(
      <AppearanceProvider>
        <Probe />
      </AppearanceProvider>,
    );

    await waitFor(() =>
      expect(screen.getByLabelText("theme").textContent).toBe("poimandres-dark-theme"),
    );
    fireEvent.click(screen.getByRole("button", { name: "Use Dracula" }));

    await waitFor(() => {
      expect(screen.getByLabelText("theme").textContent).toBe("dracula");
      expect(document.documentElement.dataset.themeId).toBe("dracula");
    });

    fireEvent.click(screen.getByRole("button", { name: "Set glass" }));
    await waitFor(() => {
      expect(api.setGlassOpacity).toHaveBeenCalledWith(65);
      expect(document.documentElement.style.getPropertyValue("--glass-opacity")).toBe("0.65");
    });
  });

  it("adopts the authoritative custom-theme identity returned by the core", async () => {
    api.duplicateTheme.mockImplementation(async () => {
      const current = await api.read();
      const source = structuredClone(current);
      const dracula = BUILT_IN_THEMES.find(({ id }) => id === "dracula");
      if (!dracula) throw new Error("Missing Dracula fixture");
      source.custom_themes.push({
        version: 1,
        id: "dracula-core-copy",
        name: "Dracula copy",
        appearance: "dark",
        colors: structuredClone(dracula.colors),
      });
      api.read.mockResolvedValue(source);
      return source;
    });

    render(
      <AppearanceProvider>
        <CoreMutationProbe />
      </AppearanceProvider>,
    );
    await waitFor(() => expect(screen.getByLabelText("custom-themes").textContent).toBe(""));
    fireEvent.click(screen.getByRole("button", { name: "Duplicate Dracula" }));

    await waitFor(() =>
      expect(screen.getByLabelText("custom-themes").textContent).toBe("dracula-core-copy"),
    );
    expect(api.duplicateTheme).toHaveBeenCalledWith("dracula");
  });
});
