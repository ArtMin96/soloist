/** Targets whose queued task mutations supersede one another rather than stacking up. */
export const APPEARANCE_MUTATION_TARGET = { glassOpacity: "glass_opacity" } as const;

export type AppearanceMutationTarget =
  (typeof APPEARANCE_MUTATION_TARGET)[keyof typeof APPEARANCE_MUTATION_TARGET];

interface AppearanceMutationQueueOptions<T> {
  write: (value: T) => Promise<T>;
  read: () => Promise<T>;
  current: () => T;
  adopt: (value: T) => void;
}

interface UpdateWaiter {
  resolve: () => void;
  reject: (error: unknown) => void;
}

interface TaskWaiter<T> {
  resolve: (value: T) => void;
  reject: (error: unknown) => void;
}

interface PendingUpdate<T> {
  kind: "update";
  projections: Array<(current: T) => T>;
  waiters: UpdateWaiter[];
}

interface PendingTask<T> {
  kind: "task";
  target?: AppearanceMutationTarget;
  command: () => Promise<T>;
  waiters: TaskWaiter<T>[];
}

type PendingMutation<T> = PendingUpdate<T> | PendingTask<T>;

export interface AppearanceMutationQueue<T> {
  update: (project: (current: T) => T) => Promise<void>;
  task: (command: () => Promise<T>, target?: AppearanceMutationTarget) => Promise<T>;
}

// One serialization boundary for both legacy whole-Appearance writes and task-shaped theme
// commands. Consecutive pending projections coalesce into one write, while each projection is
// rebased onto the latest authoritative command result before that write begins. A queued task
// naming a target is likewise replaced by the next task for that target, so a burst of repeated
// settings changes costs one round trip beyond the one already in flight.
export function createAppearanceMutationQueue<T>({
  write,
  read,
  current,
  adopt,
}: AppearanceMutationQueueOptions<T>): AppearanceMutationQueue<T> {
  const pending: PendingMutation<T>[] = [];
  let running = false;

  const reconcile = async () => {
    try {
      adopt(await read());
    } catch {
      // Keep the optimistic value. The next successful mutation or provider reload reconciles it.
    }
  };

  const drain = async () => {
    if (running) return;
    running = true;

    while (pending.length > 0) {
      const mutation = pending.shift();
      if (!mutation) break;
      try {
        if (mutation.kind === "task") {
          const stored = await mutation.command();
          adopt(stored);
          for (const waiter of mutation.waiters) waiter.resolve(stored);
          continue;
        }

        const next = mutation.projections.reduce((value, project) => project(value), current());
        adopt(await write(next));
        for (const waiter of mutation.waiters) waiter.resolve();
      } catch (error) {
        await reconcile();
        for (const waiter of mutation.waiters) waiter.reject(error);
      }
    }

    running = false;
  };

  return {
    update(project) {
      adopt(project(current()));
      return new Promise<void>((resolve, reject) => {
        const tail = pending[pending.length - 1];
        if (tail?.kind === "update") {
          tail.projections.push(project);
          tail.waiters.push({ resolve, reject });
        } else {
          pending.push({ kind: "update", projections: [project], waiters: [{ resolve, reject }] });
        }
        void drain();
      });
    },
    task(command, target) {
      return new Promise<T>((resolve, reject) => {
        // `drain` shifts a mutation off before awaiting it, so anything still queued has not begun
        // and its command can be replaced by the one that supersedes it.
        const tail = pending[pending.length - 1];
        if (target !== undefined && tail?.kind === "task" && tail.target === target) {
          tail.command = command;
          tail.waiters.push({ resolve, reject });
        } else {
          pending.push({ kind: "task", target, command, waiters: [{ resolve, reject }] });
        }
        void drain();
      });
    },
  };
}
