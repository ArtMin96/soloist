import { useCallback } from "react";
import { NullableSelect } from "@/components/settings/controls/NullableSelect";
import { SegmentedControl } from "@/components/settings/controls/SegmentedControl";
import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingSelect } from "@/components/settings/controls/SettingSelect";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { SizeStepper } from "@/components/settings/controls/SizeStepper";
import { TerminalPreview } from "@/components/settings/TerminalPreview";
import { Switch } from "@/components/ui/switch";
import {
  FONT_WEIGHT_OPTIONS,
  LETTER_SPACING_OPTIONS,
  LINE_HEIGHT_OPTIONS,
  MONO_FONT_OPTIONS,
  THEME_OPTIONS,
} from "@/lib/appearance";
import { useAppearance } from "@/store/appearanceContext";
import type {
  Appearance,
  FontWeight,
  LetterSpacing,
  LineHeight,
  TerminalAppearance,
  Theme,
} from "@/domain";

// The Appearance tab: theme + interface size, then the terminal typography, with a live preview.
// Every control reads the projected appearance and raises an immutable update through the store's
// auto-saving setter — no persistence or policy here.
export function AppearancePanel() {
  const { appearance, setAppearance } = useAppearance();
  const t = appearance.terminal;

  const set = useCallback(
    (patch: Partial<Appearance>) => setAppearance({ ...appearance, ...patch }),
    [appearance, setAppearance],
  );
  const setTerminal = useCallback(
    (patch: Partial<TerminalAppearance>) =>
      setAppearance({ ...appearance, terminal: { ...appearance.terminal, ...patch } }),
    [appearance, setAppearance],
  );

  return (
    <div className="flex flex-col">
      <SettingsSection title="Application">
        <SettingRow label="Theme" description="The application color scheme.">
          <SegmentedControl<Theme>
            value={appearance.theme}
            options={THEME_OPTIONS}
            onChange={(theme) => set({ theme })}
            ariaLabel="Theme"
          />
        </SettingRow>
        <SettingRow label="Interface size" description="Adjust the size of all interface elements.">
          <SizeStepper
            value={appearance.interface_font_scale}
            onChange={(interface_font_scale) => set({ interface_font_scale })}
            ariaLabel="interface size"
          />
        </SettingRow>
      </SettingsSection>

      <SettingsSection title="Terminal">
        <SettingRow
          label="Focus on click"
          description="Single-click the terminal to focus it instead of double-click."
        >
          <Switch
            checked={t.focus_on_click}
            onCheckedChange={(focus_on_click) => setTerminal({ focus_on_click })}
            aria-label="Focus on click"
          />
        </SettingRow>
        <SettingRow label="Font family" description="The monospace font used in the terminal.">
          <NullableSelect<string>
            value={t.font_family}
            options={MONO_FONT_OPTIONS}
            onValueChange={(font_family) => setTerminal({ font_family })}
            ariaLabel="Font family"
            className="w-44"
          />
        </SettingRow>
        <SettingRow label="Font weight" description="Weight for regular terminal text.">
          <SettingSelect
            value={t.font_weight}
            options={FONT_WEIGHT_OPTIONS}
            onValueChange={(value) => setTerminal({ font_weight: value as FontWeight })}
            ariaLabel="Font weight"
            className="w-24"
          />
        </SettingRow>
        <SettingRow label="Bold font weight" description="Weight for bold terminal text.">
          <SettingSelect
            value={t.bold_font_weight}
            options={FONT_WEIGHT_OPTIONS}
            onValueChange={(value) => setTerminal({ bold_font_weight: value as FontWeight })}
            ariaLabel="Bold font weight"
            className="w-24"
          />
        </SettingRow>
        <SettingRow label="Terminal size" description="Adjust the terminal font size.">
          <SizeStepper
            value={t.font_scale}
            onChange={(font_scale) => setTerminal({ font_scale })}
            ariaLabel="terminal size"
          />
        </SettingRow>
        <SettingRow label="Line height" description="Spacing between terminal lines.">
          <SettingSelect
            value={t.line_height}
            options={LINE_HEIGHT_OPTIONS}
            onValueChange={(value) => setTerminal({ line_height: value as LineHeight })}
            ariaLabel="Line height"
            className="w-36"
          />
        </SettingRow>
        <SettingRow label="Letter spacing" description="Spacing between terminal characters.">
          <SettingSelect
            value={t.letter_spacing}
            options={LETTER_SPACING_OPTIONS}
            onValueChange={(value) => setTerminal({ letter_spacing: value as LetterSpacing })}
            ariaLabel="Letter spacing"
            className="w-28"
          />
        </SettingRow>
      </SettingsSection>

      <div>
        <h3 className="mb-1.5 text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
          Terminal preview
        </h3>
        <TerminalPreview />
      </div>
    </div>
  );
}
