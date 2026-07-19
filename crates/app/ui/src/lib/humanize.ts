// Turning an agent-written handle into the prose a person reads. Agents name shared documents as
// slugs (good for `solo://` links and tool arguments); the panels show people sentences. Pure and
// display-only — the handle itself is never rewritten, so links, MCP calls, and exports keep
// addressing the document by the name the core stores.

// A handle written as a slug: lowercase alphanumeric words joined by `-` or `_`. The trailing `+`
// requires at least one separator, so a single word is deliberately not a slug — "research" is
// already prose and must survive untouched rather than becoming "Research".
const SLUG = /^[a-z0-9]+([-_][a-z0-9]+)+$/;

/**
 * The human-readable title for a document handle: a slug becomes a sentence
 * (`rich-editor-design` → "Rich editor design"), and any name that is not a slug — one already
 * containing spaces, capitals, or other punctuation — is returned unchanged, because the user who
 * wrote it chose how it reads.
 */
export function humanizeName(name: string): string {
  if (!SLUG.test(name)) return name;
  const words = name.replace(/[-_]/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}
