import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createToastDismissal, type ToastId } from "@/lib/toastDismissal";

const DURATION = 6000;

function clock() {
  const dismissed: ToastId[] = [];
  return { dismissed, dismissal: createToastDismissal((id) => dismissed.push(id)) };
}

describe("the in-app toast countdown", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("dismisses a toast when its time is up, and not a moment before", () => {
    const { dismissed, dismissal } = clock();

    dismissal.schedule("crash", DURATION);
    vi.advanceTimersByTime(DURATION - 1);
    expect(dismissed).toEqual([]);

    vi.advanceTimersByTime(1);
    expect(dismissed).toEqual(["crash"]);
  });

  it("holds a toast open for as long as the pointer stays on it", () => {
    const { dismissed, dismissal } = clock();

    dismissal.schedule("crash", DURATION);
    vi.advanceTimersByTime(DURATION / 2);
    dismissal.pause();
    vi.advanceTimersByTime(DURATION * 10);

    expect(dismissed).toEqual([]);
  });

  it("gives the whole duration back when the pointer leaves, not the sliver that was left", () => {
    const { dismissed, dismissal } = clock();

    dismissal.schedule("crash", DURATION);
    vi.advanceTimersByTime(DURATION - 1);
    dismissal.pause();
    dismissal.resume();

    // Resuming from what remained would have dismissed it within a millisecond.
    vi.advanceTimersByTime(DURATION - 1);
    expect(dismissed).toEqual([]);

    vi.advanceTimersByTime(1);
    expect(dismissed).toEqual(["crash"]);
  });

  it("leaves a toast that was never given a countdown on screen through a hover", () => {
    const { dismissed, dismissal } = clock();

    dismissal.pause();
    dismissal.resume();
    vi.advanceTimersByTime(DURATION * 10);

    expect(dismissed).toEqual([]);
  });

  it("stops counting for a toast the user already dismissed", () => {
    const { dismissed, dismissal } = clock();

    dismissal.schedule("crash", DURATION);
    dismissal.forget("crash");
    vi.advanceTimersByTime(DURATION * 10);

    expect(dismissed).toEqual([]);
  });

  it("restarts the countdown when the same toast is scheduled again", () => {
    const { dismissed, dismissal } = clock();

    dismissal.schedule("crash", DURATION);
    vi.advanceTimersByTime(DURATION - 1);
    dismissal.schedule("crash", DURATION);
    vi.advanceTimersByTime(DURATION - 1);
    expect(dismissed).toEqual([]);

    vi.advanceTimersByTime(1);
    expect(dismissed).toEqual(["crash"]);
  });

  it("counts each toast separately", () => {
    const { dismissed, dismissal } = clock();

    dismissal.schedule("first", DURATION);
    vi.advanceTimersByTime(DURATION / 2);
    dismissal.schedule("second", DURATION);
    vi.advanceTimersByTime(DURATION / 2);
    expect(dismissed).toEqual(["first"]);

    vi.advanceTimersByTime(DURATION / 2);
    expect(dismissed).toEqual(["first", "second"]);
  });

  it("dismisses nothing once the surface has gone away", () => {
    const { dismissed, dismissal } = clock();

    dismissal.schedule("first", DURATION);
    dismissal.schedule("second", DURATION);
    dismissal.cancel();
    vi.advanceTimersByTime(DURATION * 10);

    expect(dismissed).toEqual([]);
  });
});
