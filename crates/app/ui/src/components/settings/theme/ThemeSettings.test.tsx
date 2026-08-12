// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ThemeSettings } from "@/components/settings/theme/ThemeSettings";
import { ThemeImportDialog } from "@/components/settings/theme/ThemeImportDialog";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { writeClipboard } from "@/lib/clipboard";
import { AppearanceContext } from "@/store/appearanceContext";
import { fakeAppearanceState } from "@/test/appearanceState";
import { BUILT_IN_THEMES } from "@/theme/catalog";
import { ThemeImportConflictError } from "@/theme/io";

vi.mock("@/lib/clipboard", () => ({ writeClipboard: vi.fn(() => Promise.resolve()) }));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Settings — themes", () => {
  it("offers visual appearance modes, theme creation, import, and glass opacity", () => {
    render(<ThemeSettings />);

    expect(screen.getByRole("radiogroup", { name: "Color scheme" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "System" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "Light" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "Dark" })).toBeTruthy();
    expect(screen.getByRole("slider", { name: "Glass opacity" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Create theme" }));
    expect(screen.getByRole("dialog", { name: "Create theme" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Cancel theme editing" }));
    fireEvent.click(screen.getByRole("button", { name: "Import theme" }));
    expect(screen.getByRole("dialog", { name: "Import theme" })).toBeTruthy();
  });

  it("routes scheme, palette, and glass choices through the appearance contract", async () => {
    const state = fakeAppearanceState(DEFAULT_APPEARANCE, false);
    state.setAppearanceMode = vi.fn().mockResolvedValue(undefined);
    state.selectTheme = vi.fn().mockResolvedValue(undefined);
    state.setGlassOpacity = vi.fn().mockResolvedValue(undefined);
    render(
      <AppearanceContext value={state}>
        <ThemeSettings />
      </AppearanceContext>,
    );

    fireEvent.click(screen.getByRole("radio", { name: "Dark" }));
    fireEvent.click(screen.getByRole("button", { name: "Use Poimandres dark theme for dark" }));
    fireEvent.keyDown(screen.getByRole("slider", { name: "Glass opacity" }), {
      key: "ArrowRight",
    });

    await waitFor(() => {
      expect(state.setAppearanceMode).toHaveBeenCalledWith("dark");
      expect(state.selectTheme).toHaveBeenCalledWith("poimandres-dark-theme", "dark");
      expect(state.setGlassOpacity).toHaveBeenCalledWith(85);
    });
  });

  it("previews all advanced color roles and restores the app when editing is cancelled", async () => {
    const state = fakeAppearanceState(DEFAULT_APPEARANCE, false);
    state.beginThemeDraft = vi.fn();
    state.updateThemeDraft = vi.fn();
    state.cancelThemeDraft = vi.fn();
    render(
      <AppearanceContext value={state}>
        <ThemeSettings />
      </AppearanceContext>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Create theme" }));
    expect(state.beginThemeDraft).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("switch", { name: "Advanced colors" }));
    expect(screen.getAllByLabelText(/^Choose /)).toHaveLength(57);
    const textColor = screen.getByRole("textbox", { name: "Text" });
    fireEvent.change(textColor, { target: { value: "#000000" } });
    fireEvent.blur(textColor);
    const canvasColor = screen.getByRole("textbox", { name: "Canvas" });
    fireEvent.change(canvasColor, { target: { value: "#000000" } });
    fireEvent.blur(canvasColor);
    expect(screen.getByRole("region", { name: "Accessibility warnings" })).toBeTruthy();

    fireEvent.change(screen.getByRole("textbox", { name: "Theme name" }), {
      target: { value: "Aurora" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "Author" }), {
      target: { value: "Ada" },
    });
    await waitFor(() => expect(state.updateThemeDraft).toHaveBeenCalled());

    fireEvent.click(screen.getByRole("button", { name: "Cancel theme editing" }));
    expect(state.cancelThemeDraft).toHaveBeenCalledOnce();
    expect(screen.queryByRole("dialog", { name: "Create theme" })).toBeNull();
  });

  it("cancels a live draft when settings unmounts", () => {
    const state = fakeAppearanceState(DEFAULT_APPEARANCE, false);
    state.cancelThemeDraft = vi.fn();
    const view = render(
      <AppearanceContext value={state}>
        <ThemeSettings />
      </AppearanceContext>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Create theme" }));
    view.unmount();

    expect(state.cancelThemeDraft).toHaveBeenCalledOnce();
  });

  it("highlights role usage and inspects an app element back to its color role", async () => {
    const state = fakeAppearanceState(DEFAULT_APPEARANCE, false);
    const beginThemeDraft = vi.fn();
    state.beginThemeDraft = beginThemeDraft;
    render(
      <AppearanceContext value={state}>
        <ThemeSettings />
      </AppearanceContext>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Create theme" }));
    const draft = beginThemeDraft.mock.calls[0]?.[0];
    expect(draft).toBeTruthy();
    const appElement = document.createElement("button");
    appElement.style.backgroundColor = draft.colors.canvas;
    document.body.append(appElement);

    fireEvent.click(screen.getByRole("switch", { name: "Advanced colors" }));
    const canvasInput = screen.getByRole("textbox", { name: "Canvas" });
    const roleRow = canvasInput.parentElement;
    expect(roleRow).toBeTruthy();
    fireEvent.pointerEnter(roleRow!);
    expect(appElement.style.outline).toBe("2px solid var(--ring)");
    fireEvent.pointerLeave(roleRow!);
    expect(appElement.style.outline).toBe("");

    fireEvent.click(screen.getByRole("button", { name: "Inspect" }));
    fireEvent.click(appElement);
    await waitFor(() =>
      expect(
        (screen.getByRole("textbox", { name: "Filter theme colors" }) as HTMLInputElement).value,
      ).toBe("Canvas"),
    );
    appElement.remove();
  });

  it("minimizes the floating creator and commits its live draft", async () => {
    const state = fakeAppearanceState(DEFAULT_APPEARANCE, false);
    state.commitThemeDraft = vi.fn(async (theme) => {
      if (!theme) throw new Error("No draft");
      return theme;
    });
    render(
      <AppearanceContext value={state}>
        <ThemeSettings />
      </AppearanceContext>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Create theme" }));
    const editor = screen.getByRole("dialog", { name: "Create theme" });
    expect(editor.className).toContain("resize");

    fireEvent.change(screen.getByRole("textbox", { name: "Theme name" }), {
      target: { value: "Aurora" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "Author" }), {
      target: { value: "Ada" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Minimize theme editor" }));
    expect(screen.queryByRole("textbox", { name: "Theme name" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Expand theme editor" }));
    fireEvent.click(within(editor).getByRole("button", { name: "Create theme" }));

    await waitFor(() =>
      expect(state.commitThemeDraft).toHaveBeenCalledWith(
        expect.objectContaining({ name: "Aurora", author: "Ada", version: 1 }),
      ),
    );
    expect(screen.queryByRole("dialog", { name: "Create theme" })).toBeNull();
  });

  it("offers explicit conflict resolution when an imported ID already exists", async () => {
    const existing = BUILT_IN_THEMES[1];
    const incoming = { ...existing, name: "Imported Poimandres" };
    const onImport = vi
      .fn()
      .mockRejectedValueOnce(new ThemeImportConflictError(existing, incoming))
      .mockResolvedValueOnce(incoming);
    render(<ThemeImportDialog open onOpenChange={() => {}} onImport={onImport} />);

    fireEvent.change(screen.getByLabelText("Theme JSON"), { target: { value: "{}" } });
    fireEvent.click(screen.getByRole("button", { name: "Import theme" }));
    expect(await screen.findByText("A theme with this ID already exists")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Update Existing" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Keep Both" }));
    await waitFor(() => expect(onImport).toHaveBeenLastCalledWith("{}", "keep_both"));
  });

  it("imports a chosen JSON file and can update an existing theme", async () => {
    const builtIn = BUILT_IN_THEMES[1];
    const existing = { ...builtIn, id: "custom-poimandres", source: "custom" as const };
    const { source: _source, ...existingFile } = existing;
    void _source;
    const incoming = { ...existingFile, name: "Updated Poimandres" };
    const onImport = vi
      .fn()
      .mockRejectedValueOnce(new ThemeImportConflictError(existing, incoming))
      .mockResolvedValueOnce(incoming);
    render(<ThemeImportDialog open onOpenChange={() => {}} onImport={onImport} />);

    const json = JSON.stringify(incoming);
    const file = new File([json], "custom-poimandres.json", { type: "application/json" });
    Object.defineProperty(file, "text", { value: vi.fn().mockResolvedValue(json) });
    fireEvent.change(screen.getByLabelText("Choose theme file"), { target: { files: [file] } });
    await waitFor(() =>
      expect((screen.getByLabelText("Theme JSON") as HTMLTextAreaElement).value).toBe(json),
    );

    fireEvent.click(screen.getByRole("button", { name: "Import theme" }));
    expect(await screen.findByText("A theme with this ID already exists")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Update Existing" }));
    await waitFor(() => expect(onImport).toHaveBeenLastCalledWith(json, "replace"));
  });

  it("runs the full action set for custom themes and confirms removal", async () => {
    const source = BUILT_IN_THEMES[1];
    const custom = {
      version: source.version,
      id: "my-poimandres",
      name: "My Poimandres",
      appearance: source.appearance,
      colors: source.colors,
    };
    const appearance = { ...DEFAULT_APPEARANCE, custom_themes: [custom] };
    const state = fakeAppearanceState(appearance, false);
    const copy = { ...custom, id: "my-poimandres-copy", name: "My Poimandres copy" };
    state.beginThemeDraft = vi.fn();
    state.duplicateTheme = vi.fn().mockResolvedValue(copy);
    state.serializeTheme = vi.fn().mockReturnValue('{"version":1}');
    state.removeCustomTheme = vi.fn().mockResolvedValue(undefined);
    const createObjectUrl = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:theme");
    const revokeObjectUrl = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    const download = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    render(
      <AppearanceContext value={state}>
        <ThemeSettings />
      </AppearanceContext>,
    );

    const openActions = () =>
      fireEvent.pointerDown(screen.getByRole("button", { name: "Actions for My Poimandres" }));

    openActions();
    expect(await screen.findByRole("menuitem", { name: "Edit" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Duplicate" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Copy JSON" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Export" })).toBeTruthy();

    fireEvent.click(screen.getByRole("menuitem", { name: "Edit" }));
    expect(state.beginThemeDraft).toHaveBeenCalledWith(expect.objectContaining({ id: custom.id }));
    expect(await screen.findByRole("dialog", { name: "Edit theme" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel theme editing" }));

    openActions();
    fireEvent.click(await screen.findByRole("menuitem", { name: "Duplicate" }));
    await waitFor(() => expect(state.duplicateTheme).toHaveBeenCalledWith(custom.id));
    expect(await screen.findByRole("dialog", { name: "Edit theme" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel theme editing" }));

    openActions();
    fireEvent.click(await screen.findByRole("menuitem", { name: "Copy JSON" }));
    expect(writeClipboard).toHaveBeenCalledWith('{"version":1}');

    openActions();
    fireEvent.click(await screen.findByRole("menuitem", { name: "Export" }));
    expect(createObjectUrl).toHaveBeenCalledOnce();
    expect(download).toHaveBeenCalledOnce();
    expect(revokeObjectUrl).toHaveBeenCalledWith("blob:theme");

    openActions();
    fireEvent.click(screen.getByRole("menuitem", { name: "Remove" }));

    expect(await screen.findByRole("alertdialog", { name: "Remove theme?" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(state.removeCustomTheme).toHaveBeenCalledWith("my-poimandres"));

    createObjectUrl.mockRestore();
    revokeObjectUrl.mockRestore();
    download.mockRestore();
  });

  it("offers copy, export, and duplicate actions for immutable built-in themes", async () => {
    const state = fakeAppearanceState(DEFAULT_APPEARANCE, false);
    state.serializeTheme = vi.fn().mockReturnValue('{"version":1}');
    const createObjectUrl = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:built-in");
    const revokeObjectUrl = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    const download = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    render(
      <AppearanceContext value={state}>
        <ThemeSettings />
      </AppearanceContext>,
    );

    const openActions = () =>
      fireEvent.pointerDown(
        screen.getByRole("button", { name: "Actions for Poimandres dark theme" }),
      );
    openActions();
    expect(await screen.findByRole("menuitem", { name: "Duplicate" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Copy JSON" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Export" })).toBeTruthy();
    expect(screen.queryByRole("menuitem", { name: "Edit" })).toBeNull();
    expect(screen.queryByRole("menuitem", { name: "Remove" })).toBeNull();

    fireEvent.click(screen.getByRole("menuitem", { name: "Copy JSON" }));
    expect(writeClipboard).toHaveBeenCalledWith('{"version":1}');
    openActions();
    fireEvent.click(await screen.findByRole("menuitem", { name: "Export" }));
    expect(createObjectUrl).toHaveBeenCalledOnce();
    expect(download).toHaveBeenCalledOnce();

    createObjectUrl.mockRestore();
    revokeObjectUrl.mockRestore();
    download.mockRestore();
  });
});
