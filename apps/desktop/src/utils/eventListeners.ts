import type { UnlistenFn } from "@tauri-apps/api/event";

export type UnlistenInstaller = () => Promise<UnlistenFn>;
type DomEventOptions = boolean | AddEventListenerOptions;

export function runUnlistenFns(unlisteners: Iterable<UnlistenFn>) {
  for (const unlisten of unlisteners) {
    try {
      unlisten();
    } catch (err) {
      console.error("[event-listeners] unlisten failed", err);
    }
  }
}

export function addDomEventListener<K extends keyof DocumentEventMap>(
  target: Document,
  type: K,
  listener: (event: DocumentEventMap[K]) => void,
  options?: DomEventOptions,
): UnlistenFn;
export function addDomEventListener<K extends keyof WindowEventMap>(
  target: Window,
  type: K,
  listener: (event: WindowEventMap[K]) => void,
  options?: DomEventOptions,
): UnlistenFn;
export function addDomEventListener<K extends keyof HTMLElementEventMap>(
  target: HTMLElement,
  type: K,
  listener: (event: HTMLElementEventMap[K]) => void,
  options?: DomEventOptions,
): UnlistenFn;
export function addDomEventListener(
  target: EventTarget,
  type: string,
  listener: EventListenerOrEventListenerObject,
  options?: DomEventOptions,
): UnlistenFn;
export function addDomEventListener(
  target: EventTarget,
  type: string,
  listener: EventListenerOrEventListenerObject,
  options?: DomEventOptions,
): UnlistenFn {
  target.addEventListener(type, listener, options);
  return () => target.removeEventListener(type, listener, options);
}

export async function installUnlistenFns(installers: UnlistenInstaller[]): Promise<UnlistenFn[]> {
  const unlisteners: UnlistenFn[] = [];
  try {
    for (const install of installers) {
      unlisteners.push(await install());
    }
  } catch (err) {
    runUnlistenFns([...unlisteners].reverse());
    throw err;
  }
  return unlisteners;
}

export async function installCombinedUnlisten(installers: UnlistenInstaller[]): Promise<UnlistenFn> {
  const unlisteners = await installUnlistenFns(installers);
  let active = true;
  return () => {
    if (!active) return;
    active = false;
    runUnlistenFns(unlisteners.splice(0).reverse());
  };
}

