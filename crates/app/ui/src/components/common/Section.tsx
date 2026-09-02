import { useId, type ReactNode } from "react";
import { cn } from "@/lib/utils";

interface SectionProps {
  /** Names the region, and becomes the region's accessible name. */
  title: string;
  /** A count or short qualifier set beside the title — a tally, never a control. */
  aside?: ReactNode;
  /** A control belonging to the region, set at the end of the header line. */
  action?: ReactNode;
  /** An explanatory line under the title, held to a reading measure. */
  description?: string;
  className?: string;
  children: ReactNode;
}

/**
 * One labelled region: a quiet header naming it, above whatever it draws.
 *
 * The header stays muted and light because it names the region rather than belonging to it — the
 * region's own content should be the loudest thing in it. That weight and colour are deliberately
 * not a prop: every labelled region in the app should announce itself the same way, and a knob for
 * it would preserve the eight different headers this replaces rather than settling them.
 *
 * Sizes come from the type ramp's utilities rather than from raw values, so a region inherits the
 * ramp instead of restating it: `type-label` already pairs 11px with its 14px leading, and the
 * tracking token is the letterfit that rung is meant to carry.
 *
 * Only the header lives here. Regions frame their content differently — a bordered list, a rendered
 * document, a row of actions — so a shared frame would fight all three; those that want one wrap
 * their children in `Well`.
 */
export function Section({ title, aside, action, description, className, children }: SectionProps) {
  const headingId = useId();
  return (
    <section aria-labelledby={headingId} className={cn("flex flex-col gap-1.5", className)}>
      {/* An aside sits on the title's baseline because it is text; a control cannot, so its
          presence switches the row to centred alignment rather than hanging a button off a
          baseline it was never cut to sit on. */}
      <div className={cn("flex gap-2", action == null ? "items-baseline" : "items-center")}>
        <h3
          id={headingId}
          className="type-label min-w-0 truncate font-medium tracking-[var(--tracking-label)] text-muted-foreground"
        >
          {title}
        </h3>
        {aside != null && (
          <span className="type-label shrink-0 tabular-nums text-muted-foreground">{aside}</span>
        )}
        {action != null && <div className="ml-auto shrink-0">{action}</div>}
      </div>
      {description != null && (
        <p className="type-body max-w-[52ch] text-muted-foreground">{description}</p>
      )}
      {children}
    </section>
  );
}
