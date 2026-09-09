import { useCallback, useEffect, useState } from "react";
import bundled from "../data/mvp.json";
import type { MvpData } from "../types/mvp";

/**
 * MVP data, kept current without restarting the server.
 *
 * The bundled JSON is the initial value so the page renders instantly (and
 * still works if the dev-server API is absent, e.g. `vite preview`). The
 * endpoint then re-derives from data/field_probe/ and the hyperswitch checkout
 * and replaces it — both sources change under a running server when CI
 * refreshes the probe or someone pulls hyperswitch.
 */
export function useMvpData() {
  const [data, setData] = useState<MvpData>(bundled as MvpData);
  const [refreshedAt, setRefreshedAt] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async (opts: { force?: boolean } = {}) => {
    setLoading(true);
    try {
      const res = await fetch(`/api/mvp.json${opts.force ? "?force=1" : ""}`);
      const body = await res.json();
      if (!res.ok) throw new Error(body.error ?? `HTTP ${res.status}`);
      setData(body as MvpData);
      setRefreshedAt(res.headers.get("x-mvp-mtime") ?? "");
      // A regeneration failure still returns the previous file; report it rather
      // than letting stale numbers pass as fresh.
      setError(res.headers.get("x-mvp-refresh-error"));
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    // Re-derive when the tab regains focus: the common case is editing code or
    // pulling a repo in another window, then coming back.
    const onFocus = () => void load();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [load]);

  return { data, refreshedAt, error, loading, refresh: () => load({ force: true }) };
}
