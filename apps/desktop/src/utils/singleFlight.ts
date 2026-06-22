export function singleFlight<T>(
  loads: Map<string, Promise<T>>,
  key: string,
  load: () => Promise<T>,
): Promise<T> {
  const existing = loads.get(key);
  if (existing) return existing;
  const pending = load().finally(() => {
    if (loads.get(key) === pending) {
      loads.delete(key);
    }
  });
  loads.set(key, pending);
  return pending;
}
