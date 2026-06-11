import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Per-session form-draft persistence backed by localStorage.
 *
 * - Reads the initial value lazily on mount from `grace:<key>:<sessionId>`.
 *   If parse fails or the key is missing, falls back to `initial`.
 * - Writes are debounced (~300ms) so typing doesn't hammer localStorage.
 * - `clear()` removes the key immediately and resets to `initial` — call this
 *   after a successful submit so the draft doesn't outlive the action.
 * - `sessionId` is pinned into the storage key, never the React initializer,
 *   so switching sessions can't leak one draft into another.
 *
 * SSR-safe: every localStorage access is wrapped in try/catch.
 */
export function useSessionDraft<T>(
  sessionId: string,
  key: string,
  initial: T,
): [T, (next: T | ((prev: T) => T)) => void, () => void] {
  const storageKey = `grace:${key}:${sessionId}`;

  const [value, setValue] = useState<T>(() => {
    if (!sessionId) return initial;
    try {
      const raw = localStorage.getItem(storageKey);
      if (raw == null) return initial;
      return JSON.parse(raw) as T;
    } catch {
      return initial;
    }
  });

  // Re-hydrate if `sessionId` or `key` changes (e.g., user navigates from
  // session A to session B). Without this, the React state would be stale.
  const lastKeyRef = useRef(storageKey);
  useEffect(() => {
    if (lastKeyRef.current === storageKey) return;
    lastKeyRef.current = storageKey;
    try {
      const raw = localStorage.getItem(storageKey);
      setValue(raw == null ? initial : (JSON.parse(raw) as T));
    } catch {
      setValue(initial);
    }
    // `initial` is intentionally not in the dep array — only the key change matters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [storageKey]);

  // Debounced write.
  useEffect(() => {
    if (!sessionId) return;
    const t = setTimeout(() => {
      try {
        localStorage.setItem(storageKey, JSON.stringify(value));
      } catch {
        /* quota exceeded or storage disabled; ignore */
      }
    }, 300);
    return () => clearTimeout(t);
  }, [storageKey, sessionId, value]);

  const clear = useCallback(() => {
    try {
      localStorage.removeItem(storageKey);
    } catch {
      /* ignore */
    }
    setValue(initial);
    // `initial` change shouldn't churn this callback; depend only on key.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [storageKey]);

  return [value, setValue, clear];
}
