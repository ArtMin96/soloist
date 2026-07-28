import {
  fontWeightValue,
  letterSpacingPx,
  lineHeightValue,
  terminalFontFamily,
  terminalFontPx,
} from "@/lib/appearance";
import { ANSI_COLOR_NAMES, ansiColorLabel, terminalColors } from "@/lib/terminalPalette";
import { useAppearance } from "@/store/appearanceContext";

// Hairlines drawn in the terminal's own ink rather than a design token, because they sit on the
// terminal surface and have to hold whichever theme it is in: 12% of the ink to divide the sample
// from the swatch row, 15% to outline a swatch.
const DIVIDER_ALPHA = "1f";
const SWATCH_OUTLINE_ALPHA = "26";

// A live sample of the terminal typography and theme — the same mappings the real xterm.js
// renderer reads, so what the panel shows is what the terminal becomes. Not an emulator: a
// styled sample, cheap to repaint on every change.
export function TerminalPreview() {
  const { appearance, dark } = useAppearance();
  const t = appearance.terminal;
  const colors = terminalColors(dark);

  const style = {
    fontFamily: terminalFontFamily(t.font_family),
    fontSize: `${terminalFontPx(t.font_scale)}px`,
    fontWeight: fontWeightValue(t.font_weight),
    lineHeight: lineHeightValue(t.line_height),
    letterSpacing: `${letterSpacingPx(t.letter_spacing)}px`,
    backgroundColor: colors.background,
    color: colors.foreground,
  };

  return (
    <div className="overflow-hidden rounded-lg border border-border">
      <div style={style}>
        <div className="p-3">
          <div>$ npm run dev</div>
          <div style={{ fontWeight: fontWeightValue(t.bold_font_weight) }}>
            VITE v6 ready in 312 ms
          </div>
          <div>
            <span style={{ color: colors.cursor }}>➜</span> Local: http://localhost:5173/
          </div>
        </div>
        <ul
          aria-label="ANSI palette"
          className="grid grid-cols-8 gap-1 p-3"
          style={{ borderTop: `1px solid ${colors.foreground}${DIVIDER_ALPHA}` }}
        >
          {ANSI_COLOR_NAMES.map((name) => (
            <li
              key={name}
              title={ansiColorLabel(name)}
              aria-label={ansiColorLabel(name)}
              className="h-4 rounded-[3px]"
              // The inset hairline keeps a swatch that sits near its own background — light
              // `brightWhite`, dark `black` — readable as a shape rather than a gap in the row.
              style={{
                backgroundColor: colors[name],
                boxShadow: `inset 0 0 0 1px ${colors.foreground}${SWATCH_OUTLINE_ALPHA}`,
              }}
            />
          ))}
        </ul>
      </div>
    </div>
  );
}
