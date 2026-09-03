/** The phase of a read model whose first read may still be in flight. Closed: consumers switch exhaustively. */
export const LoadStatus = {
  Loading: "loading",
  Ready: "ready",
  Failed: "failed",
} as const;
export type LoadStatus = (typeof LoadStatus)[keyof typeof LoadStatus];

/**
 * A read model in exactly one of its three phases: nothing to show yet, a value held, or a first
 * read that failed with nothing to fall back to. A re-read that fails while a value is held stays
 * `ready` — the holder reports that through its own error field rather than dropping the value.
 */
export type Loadable<T> =
  | { readonly status: typeof LoadStatus.Loading }
  | { readonly status: typeof LoadStatus.Ready; readonly value: T }
  | { readonly status: typeof LoadStatus.Failed; readonly error: string };

// The loading phase carries nothing, so every holder can share one value. Freezing it makes the
// identity stable across renders, which keeps it out of the dependency comparisons of everything
// downstream of it.
const LOADING: Loadable<never> = Object.freeze({ status: LoadStatus.Loading });

/** Nothing to show yet: the first read for the current request has not resolved. */
export function loading<T>(): Loadable<T> {
  return LOADING;
}

/** A value is held. A later re-read keeps the last value on screen rather than regressing to loading. */
export function ready<T>(value: T): Loadable<T> {
  return { status: LoadStatus.Ready, value };
}

/** The first read failed and there is no value to show. */
export function failed<T>(error: string): Loadable<T> {
  return { status: LoadStatus.Failed, error };
}
