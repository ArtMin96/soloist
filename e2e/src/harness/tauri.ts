import { browser } from "@wdio/globals";

interface TauriBridge {
  core: { invoke(command: string, args: Record<string, unknown>): Promise<unknown> };
}

/**
 * Calls one of the app's own IPC commands from a spec, through the same bridge the UI itself uses
 * (`withGlobalTauri` is on in the e2e config overlay).
 *
 * This exists for **arrange steps only** — never to perform the behavior under test. The one thing
 * a WebDriver session genuinely cannot do is drive a native GTK file dialog, so "open a project"
 * has no clickable path here. Rather than fake the app's state, this calls the same core command
 * the dialog's own handler calls, and the assertions stay on what the window renders.
 *
 * If you reach for this to *act*, stop and click the thing instead.
 */
export async function invoke<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  const result = await browser.execute(
    async (cmd: string, a: Record<string, unknown>) =>
      await (window as unknown as { __TAURI__: TauriBridge }).__TAURI__.core.invoke(cmd, a),
    command,
    args,
  );
  return result as T;
}
