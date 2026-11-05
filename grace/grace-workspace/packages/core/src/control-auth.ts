import { createHash, randomBytes, timingSafeEqual } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

/**
 * Authentication for the supervisor's control WebSocket and the dashboard's
 * local HTTP API.
 *
 * The control socket can create and start sessions, which run agents and
 * commands with the operator's credentials, so every client must present a
 * shared token. Browsers can't set headers on a WebSocket, so the token travels
 * as a subprotocol for all clients. The dashboard dev server hands it to the
 * page in a <meta> tag, which other origins can't read.
 *
 * This file only imports node builtins because vite.config.ts bundles it
 * directly, before `pnpm build` has necessarily produced packages/core/dist.
 */

export const CONTROL_PROTOCOL = "grace.control.v1";
export const CONTROL_TOKEN_META = "grace-control-token";
const TOKEN_PROTOCOL_PREFIX = "grace.token.";
/** Must stay valid as a WebSocket subprotocol, and long enough to resist guessing. */
const TOKEN_RE = /^[A-Za-z0-9_-]{32,}$/;
const LOOPBACK_HOSTNAMES = ["localhost", "127.0.0.1", "[::1]"];

export function controlTokenPath(): string {
  return path.join(os.homedir(), ".tenxgrace", "control-token");
}

/**
 * Return the shared token: TENXGRACE_CONTROL_TOKEN if set, otherwise the
 * contents of ~/.tenxgrace/control-token, generating that file on first use.
 * The supervisor, engines and dashboard are separate processes that may start
 * in any order, so creation has to be safe to race.
 */
export function getControlToken(env: NodeJS.ProcessEnv = process.env): string {
  const fromEnv = env.TENXGRACE_CONTROL_TOKEN?.trim();
  if (fromEnv) {
    if (!TOKEN_RE.test(fromEnv)) {
      throw new Error(
        "TENXGRACE_CONTROL_TOKEN must be at least 32 characters of [A-Za-z0-9_-]"
      );
    }
    return fromEnv;
  }

  const tokenPath = controlTokenPath();
  fs.mkdirSync(path.dirname(tokenPath), { recursive: true, mode: 0o700 });

  const existing = readValidToken(tokenPath);
  if (existing) return existing;

  // Write to a private temp file, then publish it atomically so a concurrent
  // reader never sees a partial token.
  const tmpPath = `${tokenPath}.${process.pid}.${randomBytes(4).toString("hex")}`;
  fs.writeFileSync(tmpPath, randomBytes(32).toString("hex"), { mode: 0o600 });
  try {
    fs.linkSync(tmpPath, tokenPath);
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code !== "EEXIST") throw err;
    // Another process won the race. Only replace its file if it's unusable.
    if (!readValidToken(tokenPath)) {
      fs.renameSync(tmpPath, tokenPath);
      return getControlToken(env);
    }
  } finally {
    fs.rmSync(tmpPath, { force: true });
  }
  const token = readValidToken(tokenPath);
  if (!token) throw new Error(`Could not read control token at ${tokenPath}`);
  return token;
}

function readValidToken(tokenPath: string): string | null {
  let contents: string;
  try {
    contents = fs.readFileSync(tokenPath, "utf-8").trim();
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw err;
  }
  if (!TOKEN_RE.test(contents)) return null;
  if ((fs.statSync(tokenPath).mode & 0o077) !== 0) fs.chmodSync(tokenPath, 0o600);
  return contents;
}

/** Subprotocols a client offers when connecting to the control socket. */
export function controlSocketProtocols(token: string): string[] {
  return [CONTROL_PROTOCOL, `${TOKEN_PROTOCOL_PREFIX}${token}`];
}

function tokensMatch(presented: string, expected: string): boolean {
  // Hash first so timingSafeEqual gets equal-length input whatever was sent.
  const digest = (s: string) => createHash("sha256").update(s).digest();
  return timingSafeEqual(digest(presented), digest(expected));
}

function hasValidToken(protocolHeader: string | undefined, expected: string): boolean {
  if (!protocolHeader) return false;
  for (const entry of protocolHeader.split(",")) {
    const protocol = entry.trim();
    if (
      protocol.startsWith(TOKEN_PROTOCOL_PREFIX) &&
      tokensMatch(protocol.slice(TOKEN_PROTOCOL_PREFIX.length), expected)
    ) {
      return true;
    }
  }
  return false;
}

function allowedHostnames(env: NodeJS.ProcessEnv): Set<string> {
  const extra = (env.TENXGRACE_ALLOWED_HOSTS ?? "")
    .split(",")
    .map((h) => h.trim().toLowerCase())
    .filter(Boolean);
  return new Set([...LOOPBACK_HOSTNAMES, ...extra]);
}

/** True when a `host[:port]` Host header names this machine, or an opted-in host. */
export function isAllowedHostHeader(
  host: string | undefined,
  env: NodeJS.ProcessEnv = process.env
): boolean {
  if (!host) return false;
  try {
    return allowedHostnames(env).has(new URL(`http://${host}`).hostname);
  } catch {
    return false;
  }
}

/** True when a browser Origin belongs to the dashboard's allowed hosts. */
export function isAllowedOrigin(
  origin: string,
  env: NodeJS.ProcessEnv = process.env
): boolean {
  try {
    const url = new URL(origin);
    return (
      (url.protocol === "http:" || url.protocol === "https:") &&
      allowedHostnames(env).has(url.hostname)
    );
  } catch {
    return false;
  }
}

/**
 * Decide whether to accept a control socket upgrade. Non-browser clients send
 * no Origin, so the token is the actual gate. The Origin check is a second
 * layer against web pages reaching a developer's local supervisor.
 */
export function isAuthorizedControlUpgrade(
  request: { origin?: string; protocolHeader?: string },
  expectedToken: string,
  env: NodeJS.ProcessEnv = process.env
): boolean {
  if (request.origin && !isAllowedOrigin(request.origin, env)) return false;
  return hasValidToken(request.protocolHeader, expectedToken);
}

export interface LocalApiRequest {
  method?: string;
  host?: string;
  origin?: string;
  secFetchSite?: string;
}

/**
 * Same-origin check for the dashboard's /api routes, which can start agents.
 * Without it any web page could POST to them, because a cross-origin request
 * with a simple content type is still delivered to the server.
 */
export function isAllowedLocalApiRequest(request: LocalApiRequest): boolean {
  const { method = "GET", host, origin, secFetchSite } = request;
  if (secFetchSite && secFetchSite !== "same-origin" && secFetchSite !== "none") {
    return false;
  }
  if (origin) {
    try {
      if (new URL(origin).host !== host) return false;
    } catch {
      return false;
    }
    return true;
  }
  // Browsers always send Origin on these methods, so a missing one can't be
  // the dashboard.
  return method === "GET" || method === "HEAD";
}

/** Interface the control socket binds to. Loopback unless explicitly overridden. */
export function resolveControlBindHost(env: NodeJS.ProcessEnv = process.env): string {
  return env.TENXGRACE_CONTROL_HOST?.trim() || "127.0.0.1";
}
