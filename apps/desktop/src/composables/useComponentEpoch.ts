import { onBeforeUnmount } from "vue";

export interface ComponentEpoch {
  assertAlive: (epoch?: number) => boolean;
  invalidate: () => void;
  nextEpoch: () => number;
}

/**
 * Guards async UI continuations against stale component state.
 *
 * The epoch replaces scattered `seq` counters; the alive flag replaces local
 * `disposed` booleans. Any delayed focus, hydration, or lazy mount should
 * capture `nextEpoch()` and only mutate state when `assertAlive(epoch)` passes.
 */
export function withComponentEpoch(): ComponentEpoch {
  let alive = true;
  let epoch = 0;

  const api: ComponentEpoch = {
    assertAlive: (value) => alive && (value === undefined || value === epoch),
    invalidate: () => {
      epoch += 1;
    },
    nextEpoch: () => {
      epoch += 1;
      return epoch;
    },
  };

  onBeforeUnmount(() => {
    alive = false;
    epoch += 1;
  });

  return api;
}
