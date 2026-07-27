// Giving a rendered diagram a definite intrinsic size.
//
// Mermaid emits `width="100%"` with an inline `max-width:<natural>px`, which sizes correctly only
// inside a container of definite width. The pan-zoom canvas is deliberately the opposite — it is
// `position: absolute; width: max-content`, because a fit must measure the diagram's true bounds, not
// the viewport's. A percentage width has nothing to resolve against there, so it falls back to the CSS
// default replaced size of 300px and the diagram is laid out at a fraction of its real size, with
// every label squeezed to match.
//
// Stamping the `viewBox` extent onto the markup as pixels makes the size intrinsic, so the diagram
// measures the same wherever it is mounted. Responsive shrinking is unaffected: the surfaces that want
// it keep `max-width: 100%; height: auto` in CSS, which still caps a wide diagram to its container.

/** `viewBox="minX minY width height"` — the extent Mermaid computed for the drawing. */
const VIEW_BOX = /viewBox="\s*[-\d.eE+]+[\s,]+[-\d.eE+]+[\s,]+([\d.eE+]+)[\s,]+([\d.eE+]+)\s*"/;

/** The `width`/`height` presentation attributes the stamped size replaces. */
const SIZE_ATTR = /\s(?:width|height)="[^"]*"/g;

/** The `max-width` Mermaid writes into the inline style, which the stamped size supersedes. */
const MAX_WIDTH_STYLE = /\s*max-width:[^;"]*;?/;

/**
 * The index just past the opening tag's `>`, skipping any `>` that sits inside an attribute value, or
 * -1 when the tag never closes.
 */
function openingTagEnd(markup: string): number {
  let quote = "";
  for (let i = 0; i < markup.length; i += 1) {
    const char = markup[i];
    if (quote) {
      if (char === quote) quote = "";
    } else if (char === '"' || char === "'") {
      quote = char;
    } else if (char === ">") {
      return i;
    }
  }
  return -1;
}

/**
 * `svg` with its intrinsic width and height set from its own `viewBox`, so it lays out at the size
 * Mermaid drew it at rather than collapsing to the replaced-element default. Markup without a usable
 * `viewBox` is returned unchanged — there is nothing to derive a size from, and a diagram that renders
 * at the wrong size still beats one mangled by a bad rewrite.
 */
export function withIntrinsicSize(svg: string): string {
  const end = openingTagEnd(svg);
  if (end === -1) return svg;
  const open = svg.slice(0, end);
  const box = VIEW_BOX.exec(open);
  if (!box) return svg;
  const width = Number(box[1]);
  const height = Number(box[2]);
  if (!(width > 0) || !(height > 0)) return svg;
  const sized = open.replace(SIZE_ATTR, "").replace(MAX_WIDTH_STYLE, "");
  return `${sized} width="${width}" height="${height}"${svg.slice(end)}`;
}
