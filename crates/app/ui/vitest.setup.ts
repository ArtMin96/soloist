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
