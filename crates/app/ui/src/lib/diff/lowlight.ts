// Stands in for `lowlight`, which the diff viewer's core pulls in as its own fallback
// highlighter and which this app never reaches.
//
// The viewer colours through the highlighter it is handed (`registerHighlighter`), and falls back
// to its bundled one only for a language that highlighter has no grammar for. Its bundled one is
// `createLowlight(all)` — every grammar `highlight.js` publishes, close to a megabyte of them,
// linked whether or not a single line is ever coloured with one. Soloist's answer for a language
// it has no grammar for is to show the file plainly rather than colour it as something it is not,
// so the fallback has nothing to add and the megabyte buys nothing.
//
// Aliased over the real package in `vite.config.ts`. It answers the *whole* instance surface the
// real `createLowlight` returns, not only the part today's viewer reads: the viewer hands its
// registry out wholesale through `getHighlighterEngine()`, and it registers a grammar of its own
// while its module is still evaluating, so a method the stub leaves out is not a missing colour —
// it is a `TypeError` thrown before the diff chunk finishes loading, which takes the window with
// it. Each method answers the way "there is no grammar here" is expressed, so the viewer takes its
// own no-syntax path: it reads the absent tree and renders the text. A viewer that leans on the
// fallback more heavily therefore degrades to plain text, never to an error.

/** What the real package's grammar set is; nothing reads it here. */
export const all = {};

/** The real package's registry, answering as one holding no grammar at all. */
export function createLowlight() {
  return {
    // A grammar is accepted and dropped rather than kept. Registering runs the grammar's own
    // definition against a `highlight.js` instance, which is the megabyte this stub exists to
    // leave out — so there is nothing here to run it against, and nothing that could read it back.
    register: () => undefined,
    registerAlias: () => undefined,
    listLanguages: () => [],
    registered: () => false,
    highlight: () => undefined,
    highlightAuto: () => undefined,
  };
}
