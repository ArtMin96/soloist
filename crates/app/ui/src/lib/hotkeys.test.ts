import { describe, expect, it } from "vitest";
import { bindingFromEvent, bindingsEqual, formatChord } from "@/lib/hotkeys";
import type { Binding } from "@/domain";

const chord = (over: Partial<Binding>): Binding => ({
  ctrl: false,
  alt: false,
  shift: false,
  super: false,
  key: "K",
  ...over,
});

describe("formatChord", () => {
  it("orders modifiers then the key, rendering arrows as glyphs", () => {
    expect(formatChord(chord({ ctrl: true, key: "K" }))).toEqual(["Ctrl", "K"]);
    expect(formatChord(chord({ alt: true, key: "ArrowDown" }))).toEqual(["Alt", "↓"]);
    expect(formatChord(chord({ ctrl: true, shift: true, key: "=" }))).toEqual([
      "Ctrl",
      "Shift",
      "=",
    ]);
  });
});

describe("bindingFromEvent", () => {
  it("waits while only a modifier is held", () => {
    expect(bindingFromEvent({ key: "Control", ctrlKey: true } as KeyboardEvent)).toBeNull();
  });

  it("uppercases a letter so it matches the core's stored convention", () => {
    const binding = bindingFromEvent({
      key: "j",
      ctrlKey: true,
      altKey: false,
      shiftKey: false,
      metaKey: false,
    } as KeyboardEvent);
    expect(binding).toEqual({ ctrl: true, alt: false, shift: false, super: false, key: "J" });
  });
});

describe("bindingsEqual", () => {
  it("compares every field", () => {
    expect(bindingsEqual(chord({ ctrl: true }), chord({ ctrl: true }))).toBe(true);
    expect(bindingsEqual(chord({ ctrl: true }), chord({ ctrl: true, shift: true }))).toBe(false);
  });
});
