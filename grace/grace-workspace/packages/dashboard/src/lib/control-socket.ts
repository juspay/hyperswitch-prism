const CONTROL_PROTOCOL = "grace.control.v1";
const TOKEN_META = "grace-control-token";
const TOKEN_PROTOCOL_PREFIX = "grace.token.";

/**
 * Open the supervisor's control socket. The dev server injects the token into
 * the page (see vite.config.ts); browsers can't set WebSocket headers, so it's
 * sent as a subprotocol. Keep these names in sync with core/src/control-auth.ts.
 */
export function openControlSocket(url: string): WebSocket {
  const token = document
    .querySelector<HTMLMetaElement>(`meta[name="${TOKEN_META}"]`)
    ?.content;
  return new WebSocket(
    url,
    token ? [CONTROL_PROTOCOL, `${TOKEN_PROTOCOL_PREFIX}${token}`] : [CONTROL_PROTOCOL]
  );
}
