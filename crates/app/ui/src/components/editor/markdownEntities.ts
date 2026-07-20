import { MarkdownManager } from "@tiptap/markdown";

/**
 * The serializer hook this corrects. `@tiptap/markdown` declares `encodeTextForMarkdown` private, so
 * it is reachable at runtime but not through the published type — a cast is the only handle, and it
 * would keep compiling if a future release renamed or dropped the method. The runtime check in
 * {@link applyMarkdownEntityCorrection} is therefore what makes that upgrade safe: it leaves the
 * serializer untouched rather than throwing mid-save.
 */
interface MarkdownTextEncoder {
  encodeTextForMarkdown?: (text: string, node: unknown, parentNode: unknown) => string;
}

/**
 * `&` opens an HTML entity only when a name or numeric reference closes it with `;`. Anywhere else it
 * is literal text, so storing it as itself survives the next load.
 */
const ENTITY_FOLLOWS_AMP = /^(?:[a-zA-Z][a-zA-Z0-9]*|#[0-9]+|#[xX][0-9a-fA-F]+);/;

/**
 * `<` opens an HTML tag only before a tag name, a closing slash, or a declaration/instruction marker.
 * `marked` passes such a tag through as real HTML, which the editor then parses as markup rather than
 * text — so those must stay encoded or the typed characters are lost.
 */
const TAG_FOLLOWS_LT = /^[a-zA-Z!/?]/;

/** Guards against a second caller stacking another correction onto the same serializer. */
let corrected = false;

/** Whether nothing but whitespace precedes this offset on its line. */
function atLineStart(text: string, offset: number): boolean {
  const lineStart = text.lastIndexOf("\n", offset - 1) + 1;
  return text.slice(lineStart, offset).trim() === "";
}

/**
 * Undoes the serializer's HTML encoding wherever the character can stand for itself in the stored
 * Markdown, and leaves it encoded wherever the next load would read it back as markup.
 *
 * Decoding unconditionally is what makes this subtle: `&lt;div&gt;` decoded to `<div>` is re-parsed as
 * an HTML tag on the next load and the text is silently destroyed. The encoding is load-bearing
 * wherever the decoded character would open markup, and merely cosmetic everywhere else.
 */
function decodeWhereUnambiguous(encoded: string): string {
  return encoded.replace(/&(amp|lt|gt);/g, (entity, name: string, offset: number) => {
    const rest = encoded.slice(offset + entity.length);
    switch (name) {
      case "amp":
        return ENTITY_FOLLOWS_AMP.test(rest) ? entity : "&";
      case "lt":
        return TAG_FOLLOWS_LT.test(rest) ? entity : "<";
      default:
        // A `>` opening a line is a blockquote marker; mid-line it is ordinary text (`->`).
        return atLineStart(encoded, offset) ? entity : ">";
    }
  });
}

/**
 * Stops the Markdown serializer from HTML-encoding text that can safely stand for itself.
 *
 * `@tiptap/markdown` runs every non-code text node through `encodeHtmlEntities` on the way out, so a
 * document holding `&`, `<`, or `>` is *stored* as `&amp;`, `&lt;`, `&gt;`. The editor hides this —
 * loading that Markdown decodes the entities again, so the text reads correctly on screen — but the
 * store keeps the entity form, which is what an agent gets over MCP and what a template carries into
 * every document seeded from it.
 *
 * The encoder is wrapped rather than replaced so the library keeps owning what it gets right —
 * markdown-syntax escaping and the inside-code guard. Code text is returned verbatim by the encoder,
 * so it compares equal and is left alone: a literal `&amp;` inside a code block stays literal. The
 * correction lands on the prototype because the serializer resolves the method through it rather than
 * through the manager the extension exposes as storage.
 *
 * Idempotent, so an extra caller cannot stack two corrections onto one serializer.
 */
export function applyMarkdownEntityCorrection(): void {
  const proto = MarkdownManager.prototype as unknown as MarkdownTextEncoder;
  const encode = proto.encodeTextForMarkdown;
  if (typeof encode !== "function" || corrected) return;
  proto.encodeTextForMarkdown = function (text, node, parentNode) {
    const encoded = encode.call(this, text, node, parentNode);
    return encoded === text ? text : decodeWhereUnambiguous(encoded);
  };
  corrected = true;
}
