import { computed, shallowRef, type ComputedRef, type ShallowRef } from "vue";

export type LazyLoadStatus = "idle" | "loading" | "loaded" | "error";

export interface LazyLoadState<T> {
  status: ShallowRef<LazyLoadStatus>;
  error: ShallowRef<unknown | null>;
  loaded: ComputedRef<boolean>;
  loading: ComputedRef<boolean>;
  failed: ComputedRef<boolean>;
  load: () => Promise<T>;
  retry: () => Promise<T>;
  peek: () => T | null;
}

export function createLazyLoadState<T>(loader: () => Promise<T>): LazyLoadState<T> {
  const status = shallowRef<LazyLoadStatus>("idle");
  const error = shallowRef<unknown | null>(null);
  let pending: Promise<T> | null = null;
  let cached: T | null = null;
  let hasCached = false;
  let loadSeq = 0;

  async function load(): Promise<T> {
    if (hasCached) return cached as T;
    if (pending) return pending;

    const seq = ++loadSeq;
    status.value = "loading";
    error.value = null;

    const current = loader()
      .then((value) => {
        cached = value;
        hasCached = true;
        if (seq === loadSeq) {
          status.value = "loaded";
          error.value = null;
        }
        return value;
      })
      .catch((err) => {
        if (seq === loadSeq) {
          status.value = "error";
          error.value = err;
        }
        throw err;
      })
      .finally(() => {
        if (pending === current) pending = null;
      });

    pending = current;
    return current;
  }

  return {
    status,
    error,
    loaded: computed(() => status.value === "loaded"),
    loading: computed(() => status.value === "loading"),
    failed: computed(() => status.value === "error"),
    load,
    retry: load,
    peek: () => (hasCached ? cached : null),
  };
}
