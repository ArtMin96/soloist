import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { useLatestRef } from "@/store/useLatestRef";
import { PanelSettledContext } from "@/store/panelSettledContext";

/** Which of the two panels is on screen. */
export type SlidingPanel = "list" | "detail";

/** The handle naming the route on screen, on the viewport the track moves inside. */
export const PANEL_ROUTE_ATTRIBUTE = "data-panel-route";

/** The handle each panel carries, valued by which one it is — how a consumer reaches into the
 *  panel a route change just brought on screen. */
export const PANEL_ATTRIBUTE = "data-panel";

// Which settle event ends the movement. Tailwind compiles `-translate-x-full` to the `translate`
// property and expands `transition-transform` to `transform, translate, scale, rotate`, so the event
// arrives named `translate` — measured in WebKitGTK, where computed `transform` stays `none`. Both
// names are accepted so a build that compiles the class the other way cannot silently stop settling;
// they converge on one guarded completion path so one movement still settles only once.
const SETTLE_PROPERTIES = new Set(["transform", "translate"]);

/** The token the track's arrival is timed by, and so the floor for waiting the movement out. */
const SETTLE_DURATION_TOKEN = "--dur-sheet";

// How long to wait before calling the movement finished without having been told it is. Read from
// the token the transition itself is timed by, so the wait cannot drift out of step with the
// movement. An engine that cannot report the token is not running the transition either, in which
// case the two frames the caller waits first are the whole wait.
function settleCeiling(): number {
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue(SETTLE_DURATION_TOKEN)
    .trim();
  const scale = value.endsWith("ms") ? 1 : value.endsWith("s") ? 1000 : 0;
  const milliseconds = parseFloat(value) * scale;
  return Number.isFinite(milliseconds) ? milliseconds : 0;
}

interface SlidingPanelsProps {
  showing: SlidingPanel;
  list: ReactNode;
  detail: ReactNode;
  /** Called once the movement between panels has finished, in either direction. */
  onSettled: () => void;
  /**
   * Layout for the viewport the track moves inside. Defaults to filling a flex parent's remaining
   * height; a consumer laid out differently supplies its own.
   */
  className?: string;
}

/**
 * A master–detail pair and the swipe between them.
 *
 * Both panels sit side by side on a track one viewport wide, so switching is a single translation:
 * the panel arriving from the right and the one leaving to the left are the same movement, which is
 * what makes going back read as retracing it rather than as a second, unrelated animation. Because
 * that translation is a plain CSS transition rather than a scripted animation, a click during the
 * slide re-targets it mid-flight instead of queueing a second one, and the list keeps its scroll
 * position across a round trip. Present and dismiss carry the app's two sheet durations, so leaving
 * is quicker than arriving.
 *
 * The panel that is not showing is `inert` for the whole movement — out of the tab order and out of
 * the accessibility tree — so nothing off-screen can be reached or focused. Both panels stay
 * mounted, so `inert` is the only thing keeping the hidden one unreachable; `aria-hidden` is not a
 * substitute, since it would leave the panel focusable and invite the hidden-focused-element error.
 *
 * Reduced motion is deliberately not handled here. The root's global safety net already collapses
 * this transition to near-instant, and that still fires `transitionend`. Adding
 * `motion-reduce:transition-none` looks like the careful thing and breaks the primitive: it
 * suppresses the event outright, stranding whatever the consumer was holding until the movement
 * finished.
 *
 * Whether the track is moving is published to everything inside it, so content expensive enough to
 * cost frames — a rendered document, a diagram — can build itself once the panel has arrived
 * instead of competing with the movement that is bringing it on screen.
 */
export function SlidingPanels({ showing, list, detail, onSettled, className }: SlidingPanelsProps) {
  const detailShowing = showing === "detail";
  // The route the panels are rendered for. A change of `showing` is the only notice the movement
  // has started, and it has to be taken during the render that starts it: noticing it afterwards
  // would publish one frame in which the panel claimed to have already arrived.
  const [route, setRoute] = useState(showing);
  const [settled, setSettled] = useState(true);
  const pendingRouteRef = useRef<SlidingPanel | null>(showing);
  const onSettledRef = useLatestRef(onSettled);
  if (route !== showing) {
    setRoute(showing);
    setSettled(false);
  }

  useLayoutEffect(() => {
    if (!settled) pendingRouteRef.current = route;
  }, [route, settled]);

  const completeSettle = useCallback(() => {
    if (pendingRouteRef.current !== route) return;
    pendingRouteRef.current = null;
    setSettled(true);
    onSettledRef.current();
  }, [onSettledRef, route]);

  // `transitionend` is the movement's own signal, but nothing guarantees one arrives: a route can
  // change with no transition running at all, and an interrupted transition never reports. Two
  // frames give a real movement the chance to start, and the duration it is timed by bounds the
  // rest. A route change restarts the bounded wait even when the preceding movement had not settled.
  useEffect(() => {
    if (settled) return;
    let timer = 0;
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => {
        timer = window.setTimeout(completeSettle, settleCeiling());
      });
    });
    return () => {
      cancelAnimationFrame(frame);
      clearTimeout(timer);
    };
  }, [completeSettle, settled]);

  // `clip`, never `hidden`: a hidden box is still programmatically scrollable, and anything
  // reaching into a panel that has not arrived yet — a `scrollIntoView`, a focus — scrolls the
  // viewport sideways and leaves every panel offset by that much, with no scrollbar to show for
  // it. `clip` cannot be scrolled at all, so it closes the whole class rather than one caller.
  // Measured in WebKitGTK: under `hidden` a `scrollIntoView({block:"nearest"})` on an off-screen
  // control moved this box 190px; under `clip`, 0.
  return (
    <PanelSettledContext value={settled}>
      <div
        {...{ [PANEL_ROUTE_ATTRIBUTE]: showing }}
        className={cn("min-h-0 flex-1 overflow-clip", className)}
      >
        {/* The track stays `w-full` rather than sizing to its content: two full-width panels would
            make it 200% wide, and `-translate-x-full` would then move it two viewports. */}
        <div
          className={cn(
            "flex h-full w-full transition-transform",
            detailShowing
              ? "-translate-x-full duration-[var(--dur-sheet)] ease-spring"
              : "duration-[var(--dur-sheet-out)] ease-out-quint",
          )}
          onTransitionEnd={(event) => {
            // Content inside a panel animates too, and those transitions bubble through the track.
            if (event.target === event.currentTarget && SETTLE_PROPERTIES.has(event.propertyName)) {
              completeSettle();
            }
          }}
        >
          <Panel name="list" inert={detailShowing} resting={settled}>
            {list}
          </Panel>
          <Panel name="detail" inert={!detailShowing} resting={settled}>
            {detail}
          </Panel>
        </div>
      </div>
    </PanelSettledContext>
  );
}

// One slot on the track. Focusable only programmatically: it is somewhere a consumer can park focus
// on a route change when the arriving panel holds nothing better to aim at.
function Panel({
  name,
  inert,
  resting,
  children,
}: {
  name: SlidingPanel;
  inert: boolean;
  /** Whether the track has stopped, which is the only time a panel is wholly off screen. */
  resting: boolean;
  children: ReactNode;
}) {
  return (
    <div
      {...{ [PANEL_ATTRIBUTE]: name }}
      inert={inert}
      tabIndex={-1}
      className={cn(
        "flex h-full w-full min-h-0 shrink-0 flex-col outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset",
        // A panel off screen is skipped for layout and paint, so a long list stops being measured
        // every time the panel beside it forces layout — but only once the track has stopped, since
        // for the length of the movement both panels are on screen, half a viewport each, and
        // skipping the one leaving would blank it mid-slide. Its DOM, state and scroll position are
        // kept, unlike `display: none`; an engine without `content-visibility` ignores the
        // declaration and keeps laying the panel out, which is merely the cost of not having it.
        inert && resting && "[content-visibility:hidden]",
      )}
    >
      {children}
    </div>
  );
}
