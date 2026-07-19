import { MarkdownManager } from "@tiptap/markdown";
import { decodeHtmlEntities } from "@tiptap/react";

/** The serializer hook this corrects, as the library declares it internally. */
type EncodeText = (text: string, node: unknown, parentNode: unknown) => string;

/** Marks the prototype as already wrapped, so repeated imports never double-decode. */
const CORRECTED = Symbol.for("soloist.markdown-entities-corrected");

type Corrigible = {
  encodeTextForMarkdown: EncodeText;
  [CORRECTED]?: true;
};

/**
 * Stops the Markdown serializer from HTML-encoding the text it writes.
 *
 * `@tiptap/markdown` runs every non-code text node through `encodeHtmlEntities` on the way out, so a
 * document holding `&`, `<`, or `>` is *stored* as `&amp;`, `&lt;`, `&gt;`. The editor hides this —
 * loading that Markdown decodes the entities again, so the text reads correctly on screen — but the
 * store keeps the entity form, which is what an agent gets over MCP and what a template carries into
 * every document seeded from it.
 *
 * The encoder is wrapped rather than replaced so the library keeps owning what it gets right —
 * markdown-syntax escaping and the inside-code guard. Code text is returned verbatim by the encoder,
 * so it compares equal and is never decoded: a literal `&amp;` inside a code block stays literal.
 * The correction lands on the prototype because the serializer resolves the method through it rather
 * than through the manager the extension exposes as storage.
 */
export function correctMarkdownEntities(): void {
  const proto = MarkdownManager.prototype as unknown as Corrigible;
  if (proto[CORRECTED]) return;
  const encode = proto.encodeTextForMarkdown;
  proto.encodeTextForMarkdown = function (text, node, parentNode) {
    const encoded = encode.call(this, text, node, parentNode);
    return encoded === text ? text : decodeHtmlEntities(encoded);
  };
  proto[CORRECTED] = true;
}
