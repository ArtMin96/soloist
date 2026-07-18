import { browser } from "@wdio/globals";
import { WAIT } from "./waits.js";

/**
 * Waits until `predicate` holds, or throws an Error built by `describeFailure` on timeout.
 *
 * The read-and-wait screens all share this shape: poll a live DOM read until it settles, and on a
 * timeout report the last value seen rather than WebdriverIO's opaque "waitUntil timed out".
 * `describeFailure` runs only on timeout, so it can read the state the predicate left in the caller's
 * closure (the last row list, the last status), and may itself be async when the message needs one
 * more read. Defaults to the core-round-trip budget — the wait every screen here polls against.
 */
export async function waitUntilOr(
  predicate: () => Promise<boolean>,
  describeFailure: () => string | Promise<string>,
  timeout: number = WAIT.core,
): Promise<void> {
  try {
    await browser.waitUntil(predicate, { timeout });
  } catch {
    throw new Error(await describeFailure());
  }
}
