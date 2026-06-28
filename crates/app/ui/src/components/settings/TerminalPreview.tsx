import {
  fontWeightValue,
  letterSpacingPx,
  lineHeightValue,
  terminalColors,
  terminalFontFamily,
  terminalFontPx,
} from "@/lib/appearance";
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
      <div style={style} className="p-3">
        <div>$ npm run dev</div>
        <div style={{ fontWeight: fontWeightValue(t.bold_font_weight) }}>
          VITE v6 ready in 312 ms
        </div>
        <div>
          <span style={{ color: colors.cursor }}>➜</span> Local: http://localhost:5173/
        </div>
      </div>
    </div>
  );
}
