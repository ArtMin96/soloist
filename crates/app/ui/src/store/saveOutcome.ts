/**
 * What a revision-guarded editor `save` resolved to. Every `save` (scratchpad, todo, diagram,
 * template) catches its own rejection to set `conflict`/`error` state and always resolves — so this
 * is the only signal a caller gets back for whether the write actually landed. `useAutosave` reads
 * it to tell a real save from a swallowed refusal, which a resolved promise alone cannot say.
 */
export type SaveOutcome = "saved" | "refused";
