import { useCallback, useEffect, useState } from "react";
import { configTrust, onDomainEvent } from "@/api";
import type { ConfigSync, TrustReviewCommand } from "@/domain";

// An open trust review: a project's `solo.yml` changed and some commands need trust
// before they can run. `commands` shrinks as each is trusted; the dialog closes when
// none remain.
export interface TrustReview {
  project: number;
  diff: ConfigSync;
  commands: TrustReviewCommand[];
}

export interface TrustStore {
  /** The open review (the dialog is shown when non-null), or null. */
  review: TrustReview | null;
  /** Trust one command by name — the sidebar affordance and the dialog both use this. */
  trust: (project: number, name: string) => void;
  /** Trust every command in the open review, then close it. */
  trustAll: () => void;
  /** Close the review without trusting (the commands stay blocked). */
  dismiss: () => void;
}

// Trust review state (A6/A9). Grants trust through the one core gate (`config_trust`) and
// re-reads the snapshot so the now-trusted command becomes startable. A `solo.yml` change
// that needs trust (`ConfigChanged{requires_trust}`) opens the review dialog; first-open
// untrusted commands are trusted inline from the sidebar via the same `trust` action.
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

  const trust = useCallback(
    (project: number, name: string) => {
      configTrust(project, name).then(refresh).catch(reportError);
      // Drop the trusted command from an open review; close it once none remain.
      setReview((prev) => {
        if (!prev) return prev;
        const commands = prev.commands.filter((command) => command.name !== name);
        return commands.length > 0 ? { ...prev, commands } : null;
      });
    },
    [refresh, reportError],
  );

  const trustAll = useCallback(() => {
    if (!review) return;
    Promise.all(review.commands.map((command) => configTrust(review.project, command.name)))
      .then(refresh)
      .catch(reportError);
    setReview(null);
  }, [review, refresh, reportError]);

  const dismiss = useCallback(() => setReview(null), []);

  return { review, trust, trustAll, dismiss };
}
