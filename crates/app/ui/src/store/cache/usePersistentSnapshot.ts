import { useCallback, useEffect, useRef, useState } from "react";
import { type CacheKey, readSnapshot, writeSnapshot } from "@/store/cache/persistentCache";

// A fetcher may emit a fast partial value before resolving its authoritative one — e.g. the
// agent picker lists tools immediately, then fills in `--version` detection. Snapshots that
// resolve in one step ignore `emit`.
export type SnapshotFetcher<T> = (emit: (partial: T) => void) => Promise<T>;

export interface SnapshotOptions {
  /** Surface a failed revalidation (a cold cache then keeps showing nothing). */
  onError?: (reason: unknown) => void;
  /**
   * When false, mounting only seeds the cached value; the caller triggers the first
   * revalidate. The agent picker uses this so opening the app never probes agent CLIs —
   * detection runs only when the picker opens. Defaults to true.
   */
  revalidateOnMount?: boolean;
}

export interface Snapshot<T> {
  /** The cached (possibly stale) value until the first revalidation resolves, then the live one. */
  value: T | null;
  /** Re-run the fetch and reconcile to the backend (e.g. on a domain event or a picker open). */
  revalidate: () => void;
}

// Stale-while-revalidate over a persisted snapshot: seed from the last-known cached value for
// an instant paint, then revalidate against the authoritative backend (which always wins) and
// write the fresh value back. A cache miss or read failure just means no instant paint — the
// live fetch still populates it; a failed revalidation keeps the stale value on screen. The
// cache is never a second source of truth: every successful fetch overwrites it.
export function usePersistentSnapshot<T>(
  key: CacheKey,
  fetcher: SnapshotFetcher<T>,
  options?: SnapshotOptions,
): Snapshot<T> {
  const [value, setValue] = useState<T | null>(null);
  // Hold the latest fetcher/options in refs so the mount effect does not re-run (and refetch)
  // when a caller passes a fresh closure each render.
  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;
  const errorRef = useRef(options?.onError);
  errorRef.current = options?.onError;
  const revalidateOnMount = options?.revalidateOnMount ?? true;

  const revalidate = useCallback(() => {
    // A partial refines the on-screen value only while it is still empty (a cold open), so a
    // cached snapshot is never downgraded to a partial mid-revalidation.
    const emit = (partial: T) => setValue((current) => current ?? partial);
    fetcherRef.current(emit).then(
      (authoritative) => {
        setValue(authoritative);
        void writeSnapshot(key, authoritative);
      },
      (reason) => errorRef.current?.(reason),
    );
  }, [key]);

  useEffect(() => {
    let cancelled = false;
    // Seed from the cache first (instant, possibly stale) without clobbering a value the
    // revalidation may have already produced, then revalidate to the backend.
    readSnapshot<T>(key)
      .then((cached) => {
        if (!cancelled && cached !== null) setValue((current) => current ?? cached);
      })
      .finally(() => {
        if (!cancelled && revalidateOnMount) revalidate();
      });
    return () => {
      cancelled = true;
    };
  }, [key, revalidate, revalidateOnMount]);

  return { value, revalidate };
}
