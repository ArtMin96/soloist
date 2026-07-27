import {
  fontWeightValue,
  letterSpacingPx,
  lineHeightValue,
  terminalFontFamily,
  terminalFontPx,
} from "@/lib/appearance";
import { ANSI_COLOR_NAMES, ansiColorLabel, terminalColors } from "@/lib/terminalPalette";
import { useAppearance } from "@/store/appearanceContext";

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
        {/* The swatches are the one place saturated colour is not reporting status: here the
            palette itself is the subject, so the sample has to show it rather than describe it. */}
        <ul
          aria-label="ANSI palette"
          className="grid grid-cols-8 gap-1 p-3"
          style={{ borderTop: `1px solid ${colors.foreground}1f` }}
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
                boxShadow: `inset 0 0 0 1px ${colors.foreground}26`,
              }}
            />
          ))}
        </ul>
      </div>
    </div>
  );
}
