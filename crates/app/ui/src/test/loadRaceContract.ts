import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, type Mock } from "vitest";

/**
 * The slice of an `open`-then-async-read store a load-race test compares: what it shows as open
 * (`identity`, e.g. a name or handle), what it seeded the editor with (`content`), and the write
 * guard it is carrying (`revision`). Projected the same way from the live store and from a
 * resolved read, so the two are comparable with one `toEqual`.
 */
export interface LoadSnapshot {
  identity: unknown;
  content: unknown;
  revision: number | null;
}

interface LoadRaceCore<TStore, TView> {
  /** Mounts the hook under test, e.g. `() => useDiagramEditor(7)`. */
  useStore: () => TStore;
  /** The mocked async read the hook's `load` calls; resolution order is controlled here. */
  readFn: Mock;
  /** Fires a load for `target` on the live store, e.g. `(store, v) => store.open(v.name)`. */
  open: (store: TStore, target: TView) => void;
  /** Projects the live store into the identity/content/revision it currently shows. */
  snapshotOf: (store: TStore) => LoadSnapshot;
}

/** What a caller gets back from a contract that leaves the store open, to keep exercising it. */
export interface RenderedStore<TStore> {
  result: { current: TStore };
}

/**
 * Proves the "latest load request wins" guard every `open`-then-async-read store hook needs
 * (`useDiagramEditor`, `useScratchpadEditor`, `useTemplateEditor`, and any future hook shaped the
 * same way): opening a second document before the first's read resolves must discard that stale
 * resolution rather than let it land under the second document's handle. Without the guard, the
 * first document's content renders under the second's name, `mountKey` bumps and re-seeds the
 * uncontrolled editor with the wrong text, and the stale revision re-arms the write guard — so the
 * next autosave either targets the wrong content or is refused with a misleading conflict. Returns
 * the rendered hook so the caller can keep exercising it, typically to prove a following `save`
 * carries the current handle and revision — the assertion that actually closes the loop on that
 * autosave harm.
 */
export async function expectSupersededReadIsDiscarded<TStore, TView>(
  opts: LoadRaceCore<TStore, TView> & {
    /** Projects a resolved read into the identity/content/revision it would seed. */
    snapshotIn: (view: TView) => LoadSnapshot;
    first: TView;
    second: TView;
  },
): Promise<RenderedStore<TStore>> {
  const { useStore, readFn, open, snapshotOf, snapshotIn, first, second } = opts;
  let resolveFirst!: (value: TView) => void;
  readFn
    .mockReturnValueOnce(
      new Promise<TView>((resolve) => {
        resolveFirst = resolve;
      }),
    )
    .mockResolvedValueOnce(second);
  const rendered = renderHook(useStore);
  const { result } = rendered;

  act(() => open(result.current, first));
  act(() => open(result.current, second));
  const wantSecond = snapshotIn(second);
  await waitFor(() => expect(snapshotOf(result.current).content).toEqual(wantSecond.content));

  await act(async () => resolveFirst(first));

  expect(snapshotOf(result.current)).toEqual(wantSecond);
  return rendered;
}

/**
 * Proves the same guard's other half: closing (or otherwise resetting) the store before an
 * in-flight read resolves must leave it closed — the late resolution must not repopulate the
 * identity/content/revision the close just cleared, or bump `mountKey` and throw away whatever the
 * next open seeds the editor with.
 */
export async function expectCloseDiscardsInFlightRead<TStore, TView>(
  opts: LoadRaceCore<TStore, TView> & {
    close: (store: TStore) => void;
    target: TView;
    mountKeyOf: (store: TStore) => number;
  },
): Promise<void> {
  const { useStore, readFn, open, close, snapshotOf, target, mountKeyOf } = opts;
  let resolveRead!: (value: TView) => void;
  readFn.mockReturnValueOnce(
    new Promise<TView>((resolve) => {
      resolveRead = resolve;
    }),
  );
  const { result } = renderHook(useStore);
  const initialMountKey = mountKeyOf(result.current);

  act(() => open(result.current, target));
  act(() => close(result.current));

  await act(async () => resolveRead(target));

  expect(snapshotOf(result.current)).toEqual({ identity: null, content: null, revision: null });
  expect(mountKeyOf(result.current)).toBe(initialMountKey);
}
