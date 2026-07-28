// A stand-in for xterm.js under jsdom, which has no measurable surface for a real emulator to
// render into. It keeps the state the terminal hooks actually act on — the options it was built
// with and what has been written to it — so a test can assert what the emulator ended up doing
// rather than which functions were called.
//
// `options` is seeded from the constructor argument exactly as a real terminal does, which is what
// lets a suite tell the creation path from the live-restyle path: an option that only ever reaches
// the constructor leaves a mounted emulator unchanged when the setting is edited.

export class FakeTerminal {
  static instances: FakeTerminal[] = [];

  options: Record<string, unknown>;
  disposed = false;
  cols = 80;
  rows = 24;
  /** Everything written to the emulator since the last reset, in order. */
  writes: Array<string | Uint8Array> = [];

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

  focus() {}

  dispose() {
    this.disposed = true;
  }

  onData() {
    return { dispose() {} };
  }
}
