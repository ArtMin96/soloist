import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

/** Which of the two panels is on screen. */
export type SlidingPanel = "list" | "detail";

// Which settle event ends the movement. Tailwind compiles `-translate-x-full` to the `translate`
// property and expands `transition-transform` to `transform, translate, scale, rotate`, so the event
// arrives named `translate` — measured in WebKitGTK, where computed `transform` stays `none`. Both
// names are accepted so a build that compiles the class the other way cannot silently stop settling;
// settling twice is harmless, because dropping what the consumer retained is idempotent.
const SETTLE_PROPERTIES = new Set(["transform", "translate"]);

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
 */
export function SlidingPanels({ showing, list, detail, onSettled, className }: SlidingPanelsProps) {
  const detailShowing = showing === "detail";
  // `clip`, never `hidden`: a hidden box is still programmatically scrollable, and anything
  // reaching into a panel that has not arrived yet — a `scrollIntoView`, a focus — scrolls the
  // viewport sideways and leaves every panel offset by that much, with no scrollbar to show for
  // it. `clip` cannot be scrolled at all, so it closes the whole class rather than one caller.
  // Measured in WebKitGTK: under `hidden` a `scrollIntoView({block:"nearest"})` on an off-screen
  // control moved this box 190px; under `clip`, 0.
  return (
    <div data-panel-route={showing} className={cn("min-h-0 flex-1 overflow-clip", className)}>
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
            onSettled();
          }
        }}
      >
        <Panel name="list" inert={detailShowing}>
          {list}
        </Panel>
        <Panel name="detail" inert={!detailShowing}>
          {detail}
        </Panel>
      </div>
    </div>
  );
}

// One slot on the track. Focusable only programmatically: it is somewhere a consumer can park focus
// on a route change when the arriving panel holds nothing better to aim at.
function Panel({
  name,
  inert,
  children,
}: {
  name: SlidingPanel;
  inert: boolean;
  children: ReactNode;
}) {
  return (
    <div
      data-panel={name}
      inert={inert}
      tabIndex={-1}
      className="flex h-full w-full min-h-0 shrink-0 flex-col outline-none"
    >
      {children}
    </div>
  );
}
