// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";

// A recording stand-in for the platform's Web Audio: jsdom has none, and what matters is what the
// bell asks the platform to make audible — a tone that starts, stops, and reaches the speakers.
class FakeParam {
  readonly points: { value: number; at: number }[] = [];
  setValueAtTime(value: number, at: number) {
    this.points.push({ value, at });
  }
  exponentialRampToValueAtTime(value: number, at: number) {
    this.points.push({ value, at });
  }
}

class FakeNode {
  readonly connectedTo: FakeNode[] = [];
  connect(target: FakeNode) {
    this.connectedTo.push(target);
    return target;
  }
  disconnect() {}
}

class FakeOscillator extends FakeNode {
  type = "";
  readonly frequency = { value: 0 };
  started: number | null = null;
  stopped: number | null = null;
  onended: (() => void) | null = null;
  start(at: number) {
    this.started = at;
  }
  stop(at: number) {
    this.stopped = at;
  }
}

class FakeGain extends FakeNode {
  readonly gain = new FakeParam();
}

class FakeAudioContext {
  static built: FakeAudioContext[] = [];
  static oscillators: FakeOscillator[] = [];
  readonly currentTime = 0;
  readonly destination = new FakeNode();
  resumed = 0;
  constructor() {
    FakeAudioContext.built.push(this);
  }
  resume() {
    this.resumed += 1;
    return Promise.resolve();
  }
  createOscillator() {
    const oscillator = new FakeOscillator();
    FakeAudioContext.oscillators.push(oscillator);
    return oscillator;
  }
  createGain() {
    return new FakeGain();
  }
}

function installAudio(constructor: unknown) {
  Object.defineProperty(globalThis, "AudioContext", { value: constructor, configurable: true });
}

// The bell keeps one audio context for the whole app, so each case needs a fresh module.
async function freshBell() {
  vi.resetModules();
  return (await import("@/lib/bell")).playBell;
}

afterEach(() => {
  FakeAudioContext.built = [];
  FakeAudioContext.oscillators = [];
  Reflect.deleteProperty(globalThis, "AudioContext");
});

function gainOf(oscillator: FakeOscillator): FakeGain {
  const [gain] = oscillator.connectedTo;
  if (!(gain instanceof FakeGain)) throw new Error("the tone was not routed through a gain");
  return gain;
}

describe("the in-app bell", () => {
  it("sounds a short tone that reaches the speakers", async () => {
    installAudio(FakeAudioContext);
    const playBell = await freshBell();

    playBell();

    expect(FakeAudioContext.oscillators).toHaveLength(1);
    const [oscillator] = FakeAudioContext.oscillators;
    expect(oscillator.type).toBe("sine");
    expect(oscillator.frequency.value).toBeGreaterThan(0);
    expect(oscillator.started).toBe(0);
    // Brief on purpose: a bell that rings on is an alarm.
    expect(oscillator.stopped).toBeGreaterThan(0);
    expect(oscillator.stopped).toBeLessThan(0.5);

    // Reaches the output rather than dangling: oscillator -> gain -> destination.
    const [context] = FakeAudioContext.built;
    expect(gainOf(oscillator).connectedTo).toEqual([context.destination]);
  });

  it("fades in and back out instead of clicking on and off", async () => {
    installAudio(FakeAudioContext);
    const playBell = await freshBell();

    playBell();

    const values = gainOf(FakeAudioContext.oscillators[0]).gain.points.map((point) => point.value);
    expect(values.length).toBeGreaterThanOrEqual(3);
    expect(values[0]).toBeLessThan(values[1]);
    expect(values[values.length - 1]).toBeLessThan(values[1]);
  });

  it("opens one audio context however often it rings", async () => {
    installAudio(FakeAudioContext);
    const playBell = await freshBell();

    playBell();
    playBell();
    playBell();

    expect(FakeAudioContext.built).toHaveLength(1);
    expect(FakeAudioContext.oscillators).toHaveLength(3);
  });

  it("nudges a context an autoplay policy left suspended", async () => {
    installAudio(FakeAudioContext);
    const playBell = await freshBell();

    playBell();

    expect(FakeAudioContext.built[0].resumed).toBe(1);
  });

  it("stays silent, and quiet about it, where the platform has no audio", async () => {
    const playBell = await freshBell();

    expect(() => playBell()).not.toThrow();
  });

  it("gives up without throwing when the platform refuses to open a context", async () => {
    installAudio(
      class {
        constructor() {
          throw new Error("audio device busy");
        }
      },
    );
    const playBell = await freshBell();

    expect(() => playBell()).not.toThrow();
    // A second attempt must not keep retrying a device that already said no.
    expect(() => playBell()).not.toThrow();
  });

  it("keeps quiet when the tone itself fails to build", async () => {
    installAudio(
      class extends FakeAudioContext {
        override createOscillator(): FakeOscillator {
          throw new Error("no more nodes");
        }
      },
    );
    const playBell = await freshBell();

    expect(() => playBell()).not.toThrow();
  });
});
