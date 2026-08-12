import { useCallback, useEffect, useMemo, useState } from "react";
import { Download, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import type { ThemeAppearance, ThemeDefinition, ThemeFile, ThemeSelection } from "@/domain";
import { writeClipboard } from "@/lib/clipboard";
import { useAppearance } from "@/store/appearanceContext";
import { DEFAULT_THEME_ID } from "@/theme/catalog";
import { deriveThemeColors, themeColorsForAppearance } from "@/theme/derive";
import { THEME_DRAFT_ID } from "@/theme/io";
import { AppearanceModeCards } from "./AppearanceModeCards";
import { GlassOpacityControl } from "./GlassOpacityControl";
import { ThemeCard } from "./ThemeCard";
import { ThemeEditor } from "./ThemeEditor";
import { ThemeImportDialog } from "./ThemeImportDialog";

interface EditorState {
  theme: ThemeFile;
  editing: boolean;
}

function downloadJson(name: string, json: string) {
  const url = URL.createObjectURL(new Blob([json], { type: "application/json" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = `${name}.json`;
  link.click();
  URL.revokeObjectURL(url);
}

function previewPalette(
  themes: ThemeDefinition[],
  builtInThemes: ThemeDefinition[],
  selected: ThemeSelection,
  mode: ThemeAppearance,
) {
  const chosen = themes.find(({ id }) => id === selected[mode]);
  const fallback = builtInThemes.find(({ id }) => id === DEFAULT_THEME_ID);
  const palette = chosen && themeColorsForAppearance(chosen, mode);
  const fallbackPalette = fallback && themeColorsForAppearance(fallback, mode);
  if (palette) return palette;
  if (fallbackPalette) return fallbackPalette;
  throw new Error(`No ${mode} theme palette is available`);
}

export function ThemeSettings() {
  const {
    appearance,
    appliedTheme,
    beginThemeDraft,
    builtInThemes,
    cancelThemeDraft,
    commitThemeDraft,
    duplicateTheme,
    glassOpacity,
    importThemeJson,
    removeCustomTheme,
    resetGlassOpacity,
    resolvedAppearance,
    selectTheme,
    selectedThemes,
    serializeTheme,
    setAppearanceMode,
    setGlassOpacity,
    themes,
    updateThemeDraft,
  } = useAppearance();
  const [importOpen, setImportOpen] = useState(false);
  const [editor, setEditor] = useState<EditorState | null>(null);
  const [remove, setRemove] = useState<ThemeDefinition | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => () => cancelThemeDraft(), [cancelThemeDraft]);

  const lightPalette = useMemo(
    () => previewPalette(themes, builtInThemes, selectedThemes, "light"),
    [builtInThemes, selectedThemes, themes],
  );
  const darkPalette = useMemo(
    () => previewPalette(themes, builtInThemes, selectedThemes, "dark"),
    [builtInThemes, selectedThemes, themes],
  );

  const openCreate = () => {
    const colors = deriveThemeColors(
      resolvedAppearance,
      appliedTheme.colors.canvas,
      appliedTheme.colors.accent,
    );
    const theme: ThemeFile = {
      version: 1,
      id: THEME_DRAFT_ID,
      name: "",
      appearance: resolvedAppearance,
      colors,
    };
    beginThemeDraft(theme);
    setEditor({ theme, editing: false });
  };

  const closeEditor = () => {
    cancelThemeDraft();
    setEditor(null);
  };

  const saveEditor = async (theme: ThemeFile) => {
    await commitThemeDraft(theme);
    setEditor(null);
  };

  // Every handler a theme card receives keeps a stable identity so the memoized cards sit out the
  // renders the panel does while a draft is live.
  const openEdit = useCallback(
    (theme: ThemeDefinition) => {
      beginThemeDraft(theme);
      setEditor({ theme, editing: true });
    },
    [beginThemeDraft],
  );

  const previewEditor = useCallback(
    (theme: ThemeFile) => updateThemeDraft(theme),
    [updateThemeDraft],
  );

  const select = useCallback(
    (themeId: string, mode: ThemeAppearance) => void selectTheme(themeId, mode),
    [selectTheme],
  );

  const duplicate = useCallback(
    async (themeId: string) => {
      setActionError(null);
      try {
        const copy = await duplicateTheme(themeId);
        beginThemeDraft(copy);
        setEditor({ theme: copy, editing: true });
      } catch (error) {
        setActionError(
          error instanceof Error ? error.message : "The theme could not be duplicated.",
        );
      }
    },
    [beginThemeDraft, duplicateTheme],
  );

  const copy = useCallback(
    (themeId: string) => void writeClipboard(serializeTheme(themeId)),
    [serializeTheme],
  );

  const exportTheme = useCallback(
    (themeId: string) => downloadJson(themeId, serializeTheme(themeId)),
    [serializeTheme],
  );

  return (
    <>
      <section className="mb-6">
        <h2 className="px-1 text-sm font-medium text-foreground">Appearance</h2>
        <p className="mt-1 px-1 text-xs text-muted-foreground">
          Choose how Soloist looks. Use a built-in theme or make your own.
        </p>
        <div className="mt-3">
          <AppearanceModeCards
            value={appearance.theme}
            light={lightPalette}
            dark={darkPalette}
            onChange={(mode) => void setAppearanceMode(mode)}
          />
        </div>
      </section>

      <section className="mb-6">
        <div className="mb-2 flex items-center gap-2 px-1">
          <h3 className="text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
            Themes
          </h3>
          <div className="ml-auto flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={openCreate}>
              <Plus data-icon="inline-start" /> Create theme
            </Button>
            <Button variant="outline" size="sm" onClick={() => setImportOpen(true)}>
              <Download data-icon="inline-start" /> Import theme
            </Button>
          </div>
        </div>
        <div className="grid grid-cols-2 gap-2">
          {themes.map((theme) => (
            <ThemeCard
              key={theme.id}
              theme={theme}
              selected={selectedThemes}
              onSelect={select}
              onDuplicate={duplicate}
              onEdit={openEdit}
              onCopy={copy}
              onExport={exportTheme}
              onRemove={setRemove}
            />
          ))}
        </div>
        {actionError && (
          <p role="alert" className="mt-2 px-1 text-xs text-destructive">
            {actionError}
          </p>
        )}
      </section>

      <SettingsSection title="Glass opacity">
        <SettingRow
          label="Glass surfaces"
          description="Higher values make future menus, dialogs, and composer surfaces more solid."
        >
          <GlassOpacityControl
            value={glassOpacity}
            onChange={setGlassOpacity}
            onReset={() => void resetGlassOpacity()}
          />
        </SettingRow>
      </SettingsSection>

      <ThemeImportDialog
        open={importOpen}
        onOpenChange={setImportOpen}
        onImport={importThemeJson}
      />

      {editor && (
        <ThemeEditor
          key={`${editor.theme.id}-${editor.editing ? "edit" : "create"}`}
          initialTheme={editor.theme}
          editing={editor.editing}
          onPreview={previewEditor}
          onCancel={closeEditor}
          onSave={saveEditor}
        />
      )}

      <AlertDialog open={remove !== null} onOpenChange={(open) => !open && setRemove(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Remove theme?</AlertDialogTitle>
            <AlertDialogDescription>
              {remove
                ? `“${remove.name}” will be removed from Soloist.`
                : "This theme will be removed."}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => {
                if (remove) void removeCustomTheme(remove.id);
                setRemove(null);
              }}
            >
              Remove
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
