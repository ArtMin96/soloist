// The terminal's colour palette — the one place the emulator's concrete colours live. Kept as
// hex rather than the app's OKLCH design tokens because xterm.js cannot parse `oklch()`, and
// kept apart from the appearance mappings so the palette can carry a full ANSI set without
// crowding the file that maps every other closed Appearance enum to its concrete value.

// xterm's ITheme is structural; we set the fields that make program output follow the app
// surface, so a minimal shape keeps the dependency off the emulator's full type.
export interface TerminalColors {
  background: string;
  foreground: string;
  cursor: string;
  cursorAccent: string;
  selectionBackground: string;
}

// The terminal's own surface palette, tracking the app light/dark theme. This is a surface
// distinct from the app `--background` tokens (DESIGN.md). The cursor's contrast colour is
// always the surface behind it, so it is derived from the background rather than restated.
export function terminalColors(dark: boolean): TerminalColors {
  const surface = dark
    ? {
        background: "#1b1e25",
        foreground: "#e6e8ec",
        cursor: "#8ab4f8",
        selectionBackground: "#33405a",
      }
    : {
        background: "#fbfbfd",
        foreground: "#23262c",
        cursor: "#3b6fd4",
        selectionBackground: "#cfdcf5",
      };
  return { ...surface, cursorAccent: surface.background };
}
