// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { setPresence } from "@/api";
import { isWindowFocused, onWindowFocusChanged } from "@/lib/window";
import { usePresence } from "@/store/usePresence";

vi.mock("@/api", () => ({ setPresence: vi.fn() }));
vi.mock("@/lib/window", () => ({
  isWindowFocused: vi.fn(),
  onWindowFocusChanged: vi.fn(),
}));

const report = vi.mocked(setPresence);
const readFocus = vi.mocked(isWindowFocused);
const listenFocus = vi.mocked(onWindowFocusChanged);

const WEB = 1;
const API = 2;

/// The focus handler the hook registered, so a test can drive a real focus change through it.
let notifyFocus: ((focused: boolean) => void) | undefined;

beforeEach(() => {
  vi.clearAllMocks();
  notifyFocus = undefined;
  report.mockResolvedValue(undefined);
  readFocus.mockResolvedValue(false);
  listenFocus.mockImplementation((handler) => {
    notifyFocus = handler;
    return Promise.resolve(() => {});
  });
});

/// The most recent presence reported to the core.
function lastReported() {
  const calls = report.mock.calls;
  return calls.length === 0 ? undefined : calls[calls.length - 1][0];
}

describe("usePresence", () => {
  it("reports the user as away until focus is known", async () => {
    renderHook(() => usePresence(WEB));

    // Claiming focus before anything has looked would route the next alert to a toast in a window
    // that may not be on screen.
    await waitFor(() => expect(report).toHaveBeenCalled());
    expect(report.mock.calls[0][0]).toEqual({ focused: false, viewing: WEB });
  });

  it("reports focus once the window answers", async () => {
    readFocus.mockResolvedValue(true);

    renderHook(() => usePresence(WEB));

    await waitFor(() => expect(lastReported()).toEqual({ focused: true, viewing: WEB }));
  });

  it("reports a new selection", async () => {
    const { rerender } = renderHook(({ viewing }) => usePresence(viewing), {
      initialProps: { viewing: WEB },
    });
    await waitFor(() => expect(report).toHaveBeenCalled());

    rerender({ viewing: API });

    await waitFor(() => expect(lastReported()).toEqual({ focused: false, viewing: API }));
  });

  it("reports focus being lost", async () => {
    readFocus.mockResolvedValue(true);
    renderHook(() => usePresence(WEB));
    await waitFor(() => expect(lastReported()).toEqual({ focused: true, viewing: WEB }));

    act(() => notifyFocus?.(false));

    await waitFor(() => expect(lastReported()).toEqual({ focused: false, viewing: WEB }));
  });

  it("reports nobody looking when the window goes away", async () => {
    readFocus.mockResolvedValue(true);
    const { unmount } = renderHook(() => usePresence(WEB));
    await waitFor(() => expect(lastReported()).toEqual({ focused: true, viewing: WEB }));

    unmount();

    // The core outlives the window (it can hide to the tray), so a parting report of "focused"
    // would send every later alert to a toast nobody can see.
    await waitFor(() => expect(lastReported()).toEqual({ focused: false, viewing: null }));
  });

  it("reports nothing at all outside a Tauri window", async () => {
    readFocus.mockImplementation(() => {
      throw new Error("no Tauri window");
    });
    listenFocus.mockImplementation(() => {
      throw new Error("no Tauri window");
    });

    renderHook(() => usePresence(WEB));

    // A plain browser or test host has no window to observe; it must still report the default
    // rather than crash the shell.
    await waitFor(() => expect(lastReported()).toEqual({ focused: false, viewing: WEB }));
  });
});
