// A stand-in for the search addon, for the suites that mount a terminal pane to test something
// other than searching. The real addon needs an opened emulator with live selection and decoration
// services, which jsdom cannot give it without a measurable surface — so panes under those tests
// get this instead, and the search behavior itself is covered against the real addon in
// `components/terminal/terminalSearch.test.tsx`.
//
// It reports no results, which is what a pane that has never been searched should show.

export class FakeSearchAddon {
  findNext() {}
  findPrevious() {}
  clearDecorations() {}

  onDidChangeResults() {
    return { dispose() {} };
  }
}
