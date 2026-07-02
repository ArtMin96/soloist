// @vitest-environment jsdom
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Channel } from "@tauri-apps/api/core";
import type { ProcessView } from "@/domain";

// jsdom has no emulator surface, so the terminal is a write-recording fake; the hook's real
// attach / coalesce / flush logic is what runs. Instances accumulate so a test can tell the
// StrictMode-disposed terminal from the live one.
const { FakeTerminal } = vi.hoisted(() => {
  class FakeTerminal {
    static instances: FakeTerminal[] = [];
    writes: Array<string | Uint8Array> = [];
    disposed = false;
    options = {};
    cols = 80;
    rows = 24;
    constructor() {
      FakeTerminal.instances.push(this);
    }
    loadAddon() {}
    open() {}
    focus() {}
    dispose() {
      this.disposed = true;
    }
    write(data: string | Uint8Array) {
      this.writes.push(data);
    }
    onData() {
      return { dispose() {} };
    }
  }
  return { FakeTerminal };
});

vi.mock("@xterm/xterm", () => ({ Terminal: FakeTerminal }));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));
vi.mock("@xterm/addon-search", () => ({
  SearchAddon: class {
    findNext() {}
    findPrevious() {}
    clearDecorations() {}
  },
}));
vi.mock("@/lib/terminalRenderer", () => ({
  activateTerminalRenderer: () => Promise.resolve({ renderer: "dom", dispose() {} }),
}));
vi.mock("@/store/appearanceContext", async () => {
  const { DEFAULT_APPEARANCE } =
    await vi.importActual<typeof import("@/lib/appearance")>("@/lib/appearance");
  return { useAppearance: () => ({ appearance: DEFAULT_APPEARANCE, dark: true }) };
});

import { useTerminal } from "@/components/terminal/useTerminal";

const PROCESS: ProcessView = {
  id: 7,
  project: 1,
  kind: "Agent",
  label: "assistant",
  status: "Running",
  exit_code: null,
  requires_trust: false,
  resumable: false,
  ports: [],
  ready: "Ungated",
};

function Probe({ process }: { process: ProcessView }) {
  const { hostRef } = useTerminal(process);
  return <div ref={hostRef} />;
}

type ChunkChannel = Channel<Uint8Array>;

interface AttachCall {
  channel: ChunkChannel;
  token: number;
}

let attaches: AttachCall[];
let detached: number[];

async function settle(ms = 50) {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, ms));
  });
}

function deliver(call: AttachCall, text: string) {
  call.channel.onmessage(new TextEncoder().encode(text));
}

function writtenText(term: InstanceType<typeof FakeTerminal>) {
  const decoder = new TextDecoder();
  return term.writes.map((w) => (typeof w === "string" ? w : decoder.decode(w))).join("");
}

beforeEach(() => {
  FakeTerminal.instances = [];
  attaches = [];
  detached = [];
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
  // jsdom has no frame clock; a macrotask-based stand-in gives the hook's coalescing a
  // firing rAF like a visible window has.
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
    return setTimeout(() => cb(performance.now()), 0) as unknown as number;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => {
    clearTimeout(id as unknown as ReturnType<typeof setTimeout>);
  });
  let nextToken = 0;
  mockIPC((cmd, args) => {
    if (cmd === "pty_attach") {
      const token = ++nextToken;
      attaches.push({ channel: (args as { onChunk: ChunkChannel }).onChunk, token });
      return token;
    }
    if (cmd === "pty_detach") {
      detached.push((args as { token: number }).token);
    }
    return null;
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.unstubAllGlobals();
});

describe("useTerminal attach lifecycle", () => {
  // StrictMode runs mount → cleanup → mount, so two attachments race for the same pane; the
  // replay must land in the surviving emulator even when the process emits nothing afterwards
  // (an idle agent waiting for input).
  it("renders the scrollback replay after a remount when the process stays silent", async () => {
    render(
      <StrictMode>
        <Probe process={PROCESS} />
      </StrictMode>,
    );
    await settle();
    expect(attaches).toHaveLength(2);

    await act(async () => {
      deliver(attaches[0], "REPLAYED-HISTORY\r\n");
      deliver(attaches[1], "REPLAYED-HISTORY\r\n");
    });
    await settle();

    const live = FakeTerminal.instances.find((term) => !term.disposed);
    expect(live).toBeDefined();
    const text = writtenText(live as InstanceType<typeof FakeTerminal>);
    expect(text).toContain("REPLAYED-HISTORY");
    // The superseded attachment's replay must not double up in the surviving terminal.
    expect(text.match(/REPLAYED-HISTORY/g)).toHaveLength(1);
  });

  it("never writes a superseded attachment's bytes into any emulator", async () => {
    render(
      <StrictMode>
        <Probe process={PROCESS} />
      </StrictMode>,
    );
    await settle();
    expect(attaches).toHaveLength(2);

    await act(async () => {
      deliver(attaches[0], "STALE-BYTES\r\n");
    });
    await settle();

    for (const term of FakeTerminal.instances) {
      expect(writtenText(term)).not.toContain("STALE-BYTES");
    }
  });

  it("detaches with the token of its own attachment on unmount", async () => {
    const view = render(
      <StrictMode>
        <Probe process={PROCESS} />
      </StrictMode>,
    );
    await settle();
    view.unmount();
    await settle();

    // Every issued attachment is eventually detached with its own token, so a late detach
    // can never target a newer attachment.
    expect([...detached].sort((a, b) => a - b)).toEqual(attaches.map((a) => a.token));
  });
});
