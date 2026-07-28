// A stand-in for xterm.js under jsdom, which has no measurable surface for a real emulator to
// render into. It keeps the state the terminal hooks actually act on — the options it was built
// with, whether it holds keyboard focus, what is selected, and what has been pasted into it — so a
// test can assert what the emulator ended up doing rather than which functions were called.
//
// `options` is seeded from the constructor argument exactly as a real terminal does, which is what
// lets a suite tell the creation path from the live-restyle path: an option that only ever reaches
// the constructor leaves a mounted emulator unchanged when the setting is edited.

export class FakeTerminal {
  static instances: FakeTerminal[] = [];

  options: Record<string, unknown>;
  disposed = false;
  focused = false;
  cols = 80;
  rows = 24;
  /** Everything written to the emulator since the last reset, in order. */
  writes: Array<string | Uint8Array> = [];
  /** What the user has highlighted, as `getSelection` would report it. */
  selection = "";
  /** Everything handed to `paste`, in order. */
  pasted: string[] = [];

  private selectionListeners: (() => void)[] = [];

  constructor(options: Record<string, unknown> = {}) {
    this.options = { ...options };
    FakeTerminal.instances.push(this);
  }

  loadAddon() {}
  open() {}

  reset() {
    // A real reset clears the screen and the scrollback; dropping the recorded writes mirrors that,
    // so a test sees only what was replayed after it.
    this.writes = [];
  }

  write(data: string | Uint8Array) {
    this.writes.push(data);
  }

  focus() {
    this.focused = true;
  }

  dispose() {
    this.disposed = true;
  }

  onData() {
    return { dispose() {} };
  }

  onSelectionChange(listener: () => void) {
    this.selectionListeners.push(listener);
    return {
      dispose: () => {
        this.selectionListeners = this.selectionListeners.filter((each) => each !== listener);
      },
    };
  }

  hasSelection() {
    return this.selection.length > 0;
  }

  getSelection() {
    return this.selection;
  }

  paste(data: string) {
    this.pasted.push(data);
  }

  /** Drive the selection the way dragging across the surface would, listeners and all. */
  select(text: string) {
    this.selection = text;
    for (const listener of this.selectionListeners) listener();
  }

  /** The one instance a mounted pane is using — the emulator assertions are about. */
  static live(): FakeTerminal {
    const term = FakeTerminal.instances.find((instance) => !instance.disposed);
    if (!term) throw new Error("no mounted emulator");
    return term;
  }
}
