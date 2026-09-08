import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { DETAIL_BACK_ATTRIBUTE } from "@/components/common/DetailPane";
import { PANEL_ATTRIBUTE, type SlidingPanel } from "@/components/common/SlidingPanels";
import { useLatestRef } from "@/store/useLatestRef";

/**
 * The last cross-surface activation each project's board has already navigated for. A `focusNonce`
 * is one navigation, not a standing instruction — but the caller leaves it set on the props after
 * acting on it and unmounts the board whenever the user switches to another view, so a fresh board
 * would see the same activation still standing and open a detail nobody asked for. The board cannot
 * tell that case from a genuine one: an activation legitimately arrives at mount too, since opening
 * a document from a terminal mounts the board with the nonce already on its props. So the fact has
 * to be remembered somewhere that outlives the component — a ledger the board creates once at
 * module level. One entry per project the user navigates into, holding one number.
 */
export interface NavigationLedger {
  isSpent(project: number, nonce: number): boolean;
  spend(project: number, nonce: number): void;
}

/** A ledger of its own, so one board's spent activations can never silence another's. */
export function createNavigationLedger(): NavigationLedger {
  const navigated = new Map<number, number>();
  return {
    isSpent: (project, nonce) => navigated.get(project) === nonce,
    spend: (project, nonce) => {
      navigated.set(project, nonce);
    },
  };
}

export interface MasterDetailOptions<Key> {
  project: number;
  ledger: NavigationLedger;
  /** Whether `key` is in the live snapshot: gates inbound navigation (retry until present) and
   *  drops a vanished target. */
  present: (key: Key) => boolean;
  /** Selector, inside the list panel, of the control focus returns to on Back. */
  rowTrigger: (key: Key) => string;
  /** Selector, inside the list panel, of the control focus returns to after creating or cancelling. */
  createTrigger?: string;
  focusKey?: Key;
  focusNonce?: number;
  /** Before the detail opens on `key` — a row activation or an inbound nonce. */
  onOpen?: (key: Key) => void;
  /** When the route leaves the detail (Back, or the target vanished). The vanished case runs
   *  during render, so this may only set state or write idempotent refs. */
  onLeave?: () => void;
  /** When the retained key is released: after the slide-out settles, or on vanish. */
  onDrop?: (key: Key) => void;
}

export interface MasterDetail<Key> {
  /** The key the detail panel renders; retained through the slide-out until `onSettled`. */
  detailKey: Key | null;
  /** Whether the detail panel carries the create form. */
  creating: boolean;
  showing: SlidingPanel;
  open: (key: Key) => void;
  startCreate: () => void;
  back: () => void;
  onSettled: () => void;
}

/**
 * The detail panel's target. `showing` is the route — false while the panel slides back out, which
 * is what keeps the subject rendered for the length of that movement rather than blanking on the way.
 */
type DetailTarget<Key> =
  | { kind: "item"; key: Key; showing: boolean }
  | { kind: "create"; showing: boolean };

/** Where focus goes when a route change commits, and whether the target has to be scrolled to. */
interface PendingFocus {
  panel: SlidingPanel;
  within?: string;
  scroll: boolean;
}

// Moves DOM focus into the panel a route change just brought on screen, aiming at `within` when that
// panel offers it and at the panel itself when it does not. Queried by the same handles the
// end-to-end walks use, rather than threading refs down through two panels' components. The scroll
// is asked for explicitly and refused to `focus`, so bringing a row back into view stays vertical
// and neither call can drag the panel track sideways.
function focusPanel({ panel, within, scroll }: PendingFocus) {
  const root = document.querySelector<HTMLElement>(`[${PANEL_ATTRIBUTE}="${panel}"]`);
  const target = (within ? root?.querySelector<HTMLElement>(within) : null) ?? root;
  // Only a row in a list the reader has scrolled can be out of view. A control at the top of a
  // panel that has just arrived is already in view, and asking anyway costs a forced layout of the
  // whole page — measured in the hundred-millisecond range — in the commit before its first paint.
  if (scroll) target?.scrollIntoView({ block: "nearest" });
  target?.focus({ preventScroll: true });
}

/**
 * The route of a master–detail board: which panel is showing, which key the detail renders, and
 * where focus goes each time that changes. At most one key is open, so whatever the board attaches
 * to it — an edit session, a read — is told through `onOpen`/`onLeave`/`onDrop` rather than being
 * inferred from the route.
 *
 * The key is resolved against the whole snapshot, never a filtered set: filtering belongs to the
 * list panel, and searching there must not slam an open detail shut.
 */
export function useMasterDetail<Key>({
  project,
  ledger,
  present,
  rowTrigger,
  createTrigger,
  focusKey,
  focusNonce,
  onOpen,
  onLeave,
  onDrop,
}: MasterDetailOptions<Key>): MasterDetail<Key> {
  const [detail, setDetail] = useState<DetailTarget<Key> | null>(null);
  const pendingFocusRef = useRef<PendingFocus | null>(null);
  // Read after commit by the navigation effect below, whose dependencies are pinned to the
  // activation itself — a fresh closure per render would otherwise have to be one of them.
  const onOpenRef = useLatestRef(onOpen);

  const open = (key: Key) => {
    onOpenRef.current?.(key);
    setDetail({ kind: "item", key, showing: true });
    pendingFocusRef.current = {
      panel: "detail",
      within: `[${DETAIL_BACK_ATTRIBUTE}]`,
      scroll: false,
    };
  };

  const startCreate = () => {
    setDetail({ kind: "create", showing: true });
    pendingFocusRef.current = {
      panel: "detail",
      within: `[${DETAIL_BACK_ATTRIBUTE}]`,
      scroll: false,
    };
  };

  const showList = () => setDetail((current) => (current ? { ...current, showing: false } : null));

  const back = () => {
    const from = detail;
    onLeave?.();
    showList();
    if (from != null) {
      pendingFocusRef.current = {
        panel: "list",
        within: from.kind === "item" ? rowTrigger(from.key) : createTrigger,
        scroll: from.kind === "item",
      };
    }
  };

  // The subject's panel stays rendered until it has finished sliding out, so the panel leaving
  // carries the content the reader was looking at rather than blanking on the way.
  const onSettled = () => {
    if (detail == null || detail.showing) return;
    setDetail(null);
    if (detail.kind === "item") onDrop?.(detail.key);
  };

  // A route change is the one moment focus can be lost: the panel leaving goes inert, and focus left
  // inside it would fall to the document body and restart keyboard traversal at the top of the app.
  // Laid out rather than deferred, so there is no painted frame in between; read from a ref rather
  // than from the route, so a repeat navigation to the key already open still refocuses it.
  useLayoutEffect(() => {
    const pending = pendingFocusRef.current;
    if (pending == null) return;
    pendingFocusRef.current = null;
    focusPanel(pending);
  });

  // A subject can vanish from under an open detail panel — deleted, or moved out of this project.
  // There is nothing left to show or edit, so the board drops straight back to the list rather than
  // holding a panel over something that no longer exists. Only while the panel is showing: one
  // already sliding out is cleared by `onSettled` a beat later, and cutting it short would blank it
  // mid-movement. Adjusted here during render, keyed off the snapshot itself changing, so the drop
  // lands the same render the subject disappears in rather than painting a dead panel for a frame.
  if (detail?.kind === "item" && detail.showing && !present(detail.key)) {
    setDetail(null);
    onLeave?.();
    onDrop?.(detail.key);
  }

  // Whether the navigation target has actually arrived in the live snapshot. Coming from a freshly
  // mounted pane, `focusNonce` can be set before the first snapshot lands — `targetPresent` gates
  // the navigation below on that, and its own presence in the effect's deps is what makes the
  // navigation retry once the subject shows up, rather than silently missing it.
  const targetPresent = focusKey != null && present(focusKey);

  // Cross-surface navigation's inbound half: a fresh nonce opens that subject's detail panel
  // directly, so the target is on screen whatever the list panel's filter and grouping happen to
  // be. An activation already navigated for is spent, even across a remount — see `NavigationLedger`.
  useEffect(() => {
    if (focusKey == null || focusNonce == null || !targetPresent) return;
    if (ledger.isSpent(project, focusNonce)) return;
    // Marks the nonce spent before acting on it: a ledger that outlives the component, not render
    // state — render must stay replayable (Strict Mode, discarded renders), and mutating it there
    // would mark a navigation spent that never actually happened. `open` in turn writes
    // `pendingFocusRef.current`, a ref write that render itself may not make either — so the panel
    // open (and the ref-driven focus move that depends on it) genuinely belongs here, once, in
    // response to this external navigation event.
    ledger.spend(project, focusNonce);
    // eslint-disable-next-line react-hooks/set-state-in-effect -- opens in response to an external navigation event (see above); the ref write it makes cannot happen during render.
    open(focusKey);
    // Only the navigation's own trigger conditions belong here — `open` closes over the caller's
    // callbacks, fresh every render, which would re-fire this on every one.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focusNonce, focusKey, targetPresent, project]);

  return {
    detailKey: detail?.kind === "item" ? detail.key : null,
    creating: detail?.kind === "create",
    showing: detail?.showing ? "detail" : "list",
    open,
    startCreate,
    back,
    onSettled,
  };
}
