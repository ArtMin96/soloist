import { browser } from "@wdio/globals";

interface WindowBridge {
  window: { getCurrentWindow(): { isFocused(): Promise<boolean> } };
}

/**
 * Fails unless the app's window really holds focus.
 *
 * A precondition rather than an arrange step, and the only one in this harness that has to be
 * *asserted*: the core routes an alert to the desktop instead of an in-app toast whenever the
 * window is unfocused, so a walk that expects no toast would pass on lost focus for a reason that
 * has nothing to do with what it is testing — the one shape of green a mutation pass cannot catch.
 *
 * Nothing here tries to take focus, because nothing can: the window is given it by the desktop the
 * suite runs on, and a GNOME/Wayland session refuses an XWayland client focus however it asks —
 * through Tauri's own `set_focus` as readily as through `xdotool` or `wmctrl`. A display with
 * nothing else on it always grants it, which is why the remedy is a display rather than a call.
 */
export async function requireWindowFocus(): Promise<void> {
  const focused = await browser.execute(
    async () =>
      await (
        window as unknown as { __TAURI__: WindowBridge }
      ).__TAURI__.window
        .getCurrentWindow()
        .isFocused(),
  );
  if (!focused) {
    throw new Error(
      "the app window does not have focus, so every alert routes to the desktop instead of an " +
        "in-app toast and no walk here can prove anything — give the run a display of its own: " +
        "`xvfb-run -a just e2e`",
    );
  }
}
