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
// Aliased over the real package in `vite.config.ts`. Every method answers the way "there is no
// grammar here" is expressed, so the viewer takes its own no-syntax path: it reads the absent
// tree and renders the text. A future version of the viewer that leans on the fallback more
// heavily therefore degrades to plain text, never to an error.

/** What the real package's grammar set is; nothing reads it here. */
export const all = {};

/** The real package's registry, answering as one holding no grammar at all. */
export function createLowlight() {
  return {
    registered: () => false,
    highlight: () => undefined,
    highlightAuto: () => undefined,
  };
}
