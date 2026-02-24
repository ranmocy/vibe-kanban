import { useState, useEffect } from 'react';

/**
 * Global queue for staggered rendering. Ensures only one heavy component
 * mounts per idle callback, preventing the browser from being overwhelmed
 * when many items enter the viewport simultaneously.
 */
const pendingCallbacks: Array<() => void> = [];
let isProcessing = false;

function processQueue() {
  if (isProcessing || pendingCallbacks.length === 0) return;
  isProcessing = true;

  const schedule =
    typeof requestIdleCallback !== 'undefined'
      ? requestIdleCallback
      : (cb: IdleRequestCallback) => setTimeout(cb, 16);

  schedule(() => {
    const callback = pendingCallbacks.shift();
    callback?.();
    isProcessing = false;
    processQueue();
  });
}

/**
 * Defers rendering of heavy content by returning `false` initially,
 * then `true` after the browser has an idle moment. Uses a global queue
 * so that multiple deferred items mount one at a time instead of all at once.
 *
 * @param shouldDefer - Whether to defer. When `false`, returns `true` immediately.
 * @returns Whether the content is ready to render.
 */
export function useDeferredRender(shouldDefer: boolean): boolean {
  const [ready, setReady] = useState(!shouldDefer);

  useEffect(() => {
    if (!shouldDefer) {
      setReady(true);
      return;
    }

    if (ready) return;

    const callback = () => setReady(true);
    pendingCallbacks.push(callback);
    processQueue();

    return () => {
      const index = pendingCallbacks.indexOf(callback);
      if (index !== -1) pendingCallbacks.splice(index, 1);
    };
  }, [shouldDefer, ready]);

  return ready;
}
