import { useCallback, useState } from "react";

export interface GitActions {
  /** Whether the action tracked under `key` is still running. */
  busy: (key: string) => boolean;
  /** What the last refused action said, for the surface to show until it is dismissed. */
  error: string | null;
  dismissError: () => void;
  /**
   * Runs one action, tracked under `key`. Resolves true when it was carried out, false when it was
   * refused. `expected` is asked *at the refusal*, so an action the caller stopped part-way is told
   * apart from one that genuinely failed — the first is what they asked for and says nothing, the
   * second is a message they need.
   */
  run: (key: string, action: () => Promise<void>, expected?: () => boolean) => Promise<boolean>;
}

/**
 * What is in flight against a repository, and what was last refused.
 *
 * Every repository surface needs both and neither belongs to any one of them, so this is the one
 * place they are tracked. Keyed by the path, hunk, or action they belong to — never by a position
 * in a list — so a row that unmounts and comes back comes back to its own state.
 *
 * It holds no rules: what is allowed, what a refusal means, and what an action reaches are all the
 * core's answers.
 */
export function useGitActions(): GitActions {
  const [running, setRunning] = useState<ReadonlySet<string>>(new Set());
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(
    (key: string, action: () => Promise<void>, expected?: () => boolean): Promise<boolean> => {
      setRunning((keys) => new Set(keys).add(key));
      setError(null);
      return action()
        .then(() => true)
        .catch((reason: unknown) => {
          // An action the caller stopped comes back refused, because from the core's side it did
          // not finish. Saying so would be reporting their own decision back at them — but a real
          // failure of the same action still has to be said, which is why this is asked here and
          // not when the action started.
          if (expected?.() !== true) setError(String(reason));
          return false;
        })
        .finally(() =>
          setRunning((keys) => {
            const next = new Set(keys);
            next.delete(key);
            return next;
          }),
        );
    },
    [],
  );

  return {
    busy: (key: string) => running.has(key),
    error,
    dismissError: useCallback(() => setError(null), []),
    run,
  };
}
