import { beforeEach, vi } from "vitest";

// Shared test setup: browser APIs jsdom omits but components rely on. cmdk (the command
// palette) constructs a ResizeObserver on mount, which jsdom does not provide — a no-op stub
// lets observer-based components mount under test.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

if (!("ResizeObserver" in globalThis)) {
  globalThis.ResizeObserver = ResizeObserverStub as unknown as typeof ResizeObserver;
}

// cmdk scrolls the active item into view as the selection moves; jsdom has no layout, so
// `scrollIntoView` is undefined. A no-op keeps keyboard navigation working under test.
if (typeof Element !== "undefined" && !Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}

// jsdom declares `localStorage` on the global but leaves its value `undefined`, so a component that
// persists state through it throws on first render (react-resizable-panels reads a saved split
// layout as it mounts). An in-memory Storage keeps that read/write path real rather than no-op, so
// persistence assertions still mean something, and it is cleared before each test so a layout saved
// by one case never leaks into the next.
class MemoryStorage {
  private entries = new Map<string, string>();

  get length(): number {
    return this.entries.size;
  }

  clear(): void {
    this.entries.clear();
  }

  getItem(key: string): string | null {
    return this.entries.get(key) ?? null;
  }

  key(index: number): string | null {
    return [...this.entries.keys()][index] ?? null;
  }

  removeItem(key: string): void {
    this.entries.delete(key);
  }

  setItem(key: string, value: string): void {
    this.entries.set(key, String(value));
  }
}

if (globalThis.localStorage == null) {
  const storage = new MemoryStorage();
  Object.defineProperty(globalThis, "localStorage", {
    value: storage as unknown as Storage,
    configurable: true,
  });
  beforeEach(() => storage.clear());
}

// The titlebar reads and drives the OS window through `@/lib/window`; jsdom has no Tauri
// runtime behind it. Stub that one platform boundary with harmless no-ops so window-aware
// components mount under test — the live window behavior is covered by manual/e2e checks.
vi.mock("@/lib/window", () => ({
  minimizeWindow: () => Promise.resolve(),
  toggleMaximizeWindow: () => Promise.resolve(),
  closeWindow: () => Promise.resolve(),
  isWindowMaximized: () => Promise.resolve(false),
  onWindowResized: () => Promise.resolve(() => {}),
}));
