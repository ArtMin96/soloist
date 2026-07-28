// The terminal's colour palette — the one place the emulator's concrete colours live. Kept as
// hex rather than the app's OKLCH design tokens because xterm.js cannot parse `oklch()`, and
// kept apart from the appearance mappings so the palette can carry a full ANSI set without
// crowding the file that maps every other closed Appearance enum to its concrete value.

// The ANSI slots xterm renders program colour into, in the order the wire protocol numbers
// them (0-7 normal, 8-15 bright). One list so the palette, the settings preview and the
// contrast checks all walk the same set.
export const ANSI_COLOR_NAMES = [
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
  "brightBlack",
  "brightRed",
  "brightGreen",
  "brightYellow",
  "brightBlue",
  "brightMagenta",
  "brightCyan",
  "brightWhite",
] as const;

export type AnsiColorName = (typeof ANSI_COLOR_NAMES)[number];

// The surface colours, distinct from the ANSI set: they dress the emulator itself rather than
// carrying program output.
interface TerminalSurfaceColors {
  background: string;
  foreground: string;
  cursor: string;
  cursorAccent: string;
  selectionBackground: string;
  selectionInactiveBackground: string;
  scrollbarSliderBackground: string;
  scrollbarSliderHoverBackground: string;
  scrollbarSliderActiveBackground: string;
  overviewRulerBorder: string;
}

// xterm's ITheme is structural; we set the fields that make program output follow the app
// surface, so a minimal shape keeps the dependency off the emulator's full type.
export type TerminalColors = TerminalSurfaceColors & Record<AnsiColorName, string>;

// The floor xterm lifts a foreground to when the program's own colour would be unreadable on
// the cell behind it. It is the runtime backstop for colour we do not choose: 256-colour and
// truecolor output, and the two ANSI slots whose role is the surface end of their own theme
// (below). The renderer measures against the cell's real background, including a selection.
export const TERMINAL_MINIMUM_CONTRAST_RATIO = 4.5;

// Each hue is the app's own signal hue, so the terminal reads as one of the instruments rather
// than a foreign surface: red is the crashed red, amber the transition amber, green the running
// green, blue the azure accent. Cyan bridges green to azure; magenta sits clear of the violet
// band the design system rejects. Black and white ride the cool-slate neutral.
//
// Every slot clears 4.5:1 against its own background except the one that *is* the surface end of
// its theme — light `white`/`brightWhite`, dark `black`. Those exist to be the near-background
// tone (`\e[47m` must paint a pale panel, not a mid-grey one), so forcing them to 4.5:1 would
// invert what the slot means. `TERMINAL_MINIMUM_CONTRAST_RATIO` covers them when a program uses
// one as a foreground.
//
// Bright is the more emphatic set, not merely the lighter one: `drawBoldTextInBrightColors`
// defaults on, so bold output renders here and must never be less legible than its normal twin.
// The achromatic pair is the exception ANSI itself fixes — `brightBlack` is the dim slot.
//
// `brightBlack` carries extra headroom against the terminal background because it is measured
// against the *selection* background too. A selection is a 30% wash of the selection colour over
// the surface, which costs every slot a little contrast; the dim slot is the one with no margin
// to spare, and it is also the slot CLI output leans on hardest. Holding it above the floor on
// all three backgrounds keeps dim text from visibly shifting colour the moment it is selected.
const LIGHT_ANSI: Record<AnsiColorName, string> = {
  black: "#20242a",
  red: "#be433c",
  green: "#1e7d3e",
  yellow: "#996000",
  blue: "#0c71b2",
  magenta: "#a04d9e",
  cyan: "#00797f",
  white: "#c7c9cd",
  brightBlack: "#686e74",
  brightRed: "#af0b15",
  brightGreen: "#00652c",
  brightYellow: "#7b4c00",
  brightBlue: "#005a91",
  brightMagenta: "#8f2a8f",
  brightCyan: "#006166",
  brightWhite: "#ffffff",
};

const DARK_ANSI: Record<AnsiColorName, string> = {
  black: "#30353c",
  red: "#f57469",
  green: "#54ad6a",
  yellow: "#cc8f40",
  blue: "#4ba2e5",
  magenta: "#d27dd0",
  cyan: "#1eacb2",
  white: "#cfd2d7",
  brightBlack: "#8b9197",
  brightRed: "#ffa89d",
  brightGreen: "#7fd091",
  brightYellow: "#eeb46e",
  brightBlue: "#7fc5ff",
  brightMagenta: "#f1a5ee",
  brightCyan: "#61cfd4",
  brightWhite: "#ffffff",
};

// xterm paints its own scrollbar slider from the theme rather than through CSS, so these are
// what keep the terminal's scrollbar on the app's overlay rail (22% / 38% of the ink) instead
// of xterm's own 20% / 40% default. The pressed step keeps the emulator's 50%.
const SLIDER_ALPHA = { rest: "38", hover: "61", active: "80" };

// The terminal's own surface palette, tracking the app light/dark theme. This is a surface
// distinct from the app `--background` tokens. The cursor's contrast colour is always the
// surface behind it, so it is derived from the background rather than restated.
export function terminalColors(dark: boolean): TerminalColors {
  const surface = dark
    ? {
        background: "#1b1e25",
        foreground: "#e6e8ec",
        cursor: "#8ab4f8",
        selectionBackground: "#33405a",
        // The unemphasized selection: the same tone with the azure taken out, so an unfocused
        // window's selection reads as a neutral wash.
        selectionInactiveBackground: "#3e4043",
        // Separates the overview ruler from the output it summarizes. xterm leaves this black
        // when unset, which on the light surface draws a hard rule down the pane's edge.
        overviewRulerBorder: "#2b2f38",
      }
    : {
        background: "#fbfbfd",
        foreground: "#23262c",
        cursor: "#3b6fd4",
        selectionBackground: "#cfdcf5",
        selectionInactiveBackground: "#d9dcdf",
        overviewRulerBorder: "#e2e4e9",
      };
  const ansi = dark ? DARK_ANSI : LIGHT_ANSI;
  return {
    ...surface,
    ...ansi,
    cursorAccent: surface.background,
    scrollbarSliderBackground: `${surface.foreground}${SLIDER_ALPHA.rest}`,
    scrollbarSliderHoverBackground: `${surface.foreground}${SLIDER_ALPHA.hover}`,
    scrollbarSliderActiveBackground: `${surface.foreground}${SLIDER_ALPHA.active}`,
  };
}

// The colours the search addon paints every match in, and the one match currently stepped to.
// Passing them is also what makes the addon report its match counts at all — it suppresses its
// results event whenever the caller asks for no decorations.
export interface SearchDecorationColors {
  matchBackground: string;
  matchBorder: string;
  matchOverviewRuler: string;
  activeMatchBackground: string;
  activeMatchBorder: string;
  activeMatchColorOverviewRuler: string;
}

// A match is a found thing and the active match is the selected one, so the set stays inside the
// app's two colour roles: an unemphasized slate wash for every match, the azure accent for the one
// the user is standing on. No third hue is introduced, which keeps saturated colour meaning process
// status and nothing else.
//
// The two washes are deliberately quiet — each is the faintest tint that still reads against the
// terminal surface — because they tint live output rather than replacing it. What separates active
// from inactive is therefore the border, not the fill: the accent border clears its own fill by 3:1
// in both themes, so the active match stays identifiable in a grayscale screenshot and to a
// colour-blind reader, where the two fills alone differ by little. Every colour is emitted as hex
// because the emulator cannot parse the app's OKLCH tokens.
//
// The decoration replaces the cell's background before the renderer's contrast pass, so
// `TERMINAL_MINIMUM_CONTRAST_RATIO` still governs program colour drawn over a match; these
// values are chosen so the default foreground clears 4.5:1 without needing that backstop.
const LIGHT_SEARCH: SearchDecorationColors = {
  matchBackground: "#d3d8e0",
  matchBorder: "#667994",
  matchOverviewRuler: "#8493a9",
  activeMatchBackground: "#a8c3ef",
  activeMatchBorder: "#2456ad",
  activeMatchColorOverviewRuler: "#2456ad",
};

const DARK_SEARCH: SearchDecorationColors = {
  matchBackground: "#2f3741",
  matchBorder: "#6f819b",
  matchOverviewRuler: "#596a81",
  activeMatchBackground: "#18468f",
  activeMatchBorder: "#8ab4f8",
  activeMatchColorOverviewRuler: "#8ab4f8",
};

export function searchDecorationColors(dark: boolean): SearchDecorationColors {
  return dark ? DARK_SEARCH : LIGHT_SEARCH;
}

// "brightBlack" → "Bright black", naming a swatch in the settings palette preview.
export function ansiColorLabel(name: AnsiColorName): string {
  const spaced = name.replace(/([A-Z])/g, " $1").toLowerCase();
  return spaced[0].toUpperCase() + spaced.slice(1);
}
