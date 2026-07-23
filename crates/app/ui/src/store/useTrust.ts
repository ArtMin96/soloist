import { useCallback, useEffect, useState } from "react";
import { configCommandReview, configTrust, onDomainEvent } from "@/api";
import type { ConfigSync, TrustReviewCommand } from "@/domain";

// An open trust review: commands that need trust before they can run, shown with what
// each would run. `commands` shrinks as each is trusted; the dialog closes when none
// remain. `diff` is the `solo.yml` change that raised the review, or null when the user
// asked to see one command.
export interface TrustReview {
  project: number;
  diff: ConfigSync | null;
  commands: TrustReviewCommand[];
}

export interface TrustStore {
  /** The open review (the dialog is shown when non-null), or null. */
  review: TrustReview | null;
  /** Open the review for one command — what every trust affordance outside the dialog does. */
  requestReview: (project: number, name: string) => void;
  /** Trust one reviewed command by name; the dialog's per-command grant. */
  trust: (project: number, name: string) => void;
  /** Trust every command in the open review, then close it. */
  trustAll: () => void;
  /** Close the review without trusting (the commands stay blocked). */
  dismiss: () => void;
}

// Trust review state (A6/A9). Grants trust through the one core gate (`config_trust`) and
// re-reads the snapshot so the now-trusted command becomes startable. A `solo.yml` change
// that needs trust (`ConfigChanged{requires_trust}`) opens the review dialog; so does the
// sidebar or palette affordance, via `requestReview` — a grant is never made from a surface
// that shows only the command's name, since the name is chosen by the same file as the
// command it stands for.
export function useTrust(refresh: () => void, reportError: (reason: unknown) => void): TrustStore {
  const [review, setReview] = useState<TrustReview | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    onDomainEvent((event) => {
      if (event.type === "ConfigChanged" && event.requires_trust) {
        setReview({ project: event.project, diff: event.diff, commands: event.commands });
      }
    })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(reportError);
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [reportError]);

  const requestReview = useCallback(
    (project: number, name: string) => {
      configCommandReview(project, name)
        .then((command) => {
          // A command that left the file between the click and the read has nothing to
          // authorize; say so rather than opening an empty dialog or silently doing nothing.
          if (!command) {
            reportError(new Error(`${name} is no longer in this project’s solo.yml`));
            return;
          }
          setReview({ project, diff: null, commands: [command] });
        })
        .catch(reportError);
    },
    [reportError],
  );

  const trust = useCallback(
    (project: number, name: string) => {
      // Only mutate the review once trust actually applied — a failed grant leaves the
      // command in the dialog (still blocked) and surfaces the error, rather than
      // silently dropping it.
      const reviewed = review?.commands.find((command) => command.name === name);
      if (!reviewed) {
        reportError(new Error(`${name} is no longer in the open trust review`));
        return;
      }
      configTrust(project, name, reviewed.variant_hash)
        .then(() => {
          refresh();
          setReview((prev) => {
            if (!prev) return prev;
            const commands = prev.commands.filter(
              (command) => command.name !== name || command.variant_hash !== reviewed.variant_hash,
            );
            return commands.length > 0 ? { ...prev, commands } : null;
          });
        })
        .catch(reportError);
    },
    [review, refresh, reportError],
  );

  const trustAll = useCallback(() => {
    if (!review) return;
    // Close the dialog only after every grant resolved; a failure keeps it open.
    Promise.all(
      review.commands.map((command) =>
        configTrust(review.project, command.name, command.variant_hash),
      ),
    )
      .then(() => {
        refresh();
        setReview((prev) => {
          if (!prev || prev.project !== review.project) return prev;
          const granted = new Set(
            review.commands.map((command) => `${command.name}\0${command.variant_hash}`),
          );
          const commands = prev.commands.filter(
            (command) => !granted.has(`${command.name}\0${command.variant_hash}`),
          );
          return commands.length > 0 ? { ...prev, commands } : null;
        });
      })
      .catch(reportError);
  }, [review, refresh, reportError]);

  const dismiss = useCallback(() => setReview(null), []);

  return { review, requestReview, trust, trustAll, dismiss };
}
