export const DEFAULT_WS_PORT = "3142";

export const CONTROL_WS_PORT: string =
  (import.meta.env.VITE_WS_PORT as string | undefined) ?? DEFAULT_WS_PORT;

const TOKEN_KEY = "grace.controlToken";

/**
 * The supervisor prints a `/#token=...` link at boot. Keep the token and drop
 * it from the address bar. Deliberately not injected at build time: anyone who
 * can load the bundle would then have it.
 */
function controlToken(): string {
  const fromLink = new URLSearchParams(location.hash.slice(1)).get("token");
  try {
    if (fromLink) {
      localStorage.setItem(TOKEN_KEY, fromLink);
      history.replaceState(null, "", location.pathname + location.search);
    }
    return fromLink ?? localStorage.getItem(TOKEN_KEY) ?? "";
  } catch {
    return fromLink ?? "";
  }
}

export const CONTROL_WS_URL = `ws://${location.hostname}:${CONTROL_WS_PORT}?token=${encodeURIComponent(controlToken())}`;
