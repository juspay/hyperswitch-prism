import { createHash, randomBytes, timingSafeEqual } from "node:crypto";
import fs from "node:fs";
import type { IncomingMessage } from "node:http";
import path from "node:path";

import { tenxgraceHome } from "./utils.js";

const LOOPBACK_HOSTNAMES = new Set(["localhost", "127.0.0.1", "[::1]"]);

/**
 * Shared secret for the supervisor's control WebSocket. TENXGRACE_WS_TOKEN
 * wins; otherwise it's generated once into an owner-only file so the
 * supervisor and its engine children agree without any extra wiring.
 */
export function loadControlToken(): string {
  if (process.env.TENXGRACE_WS_TOKEN) return process.env.TENXGRACE_WS_TOKEN;
  const file = path.join(tenxgraceHome(), "ws-token");
  if (!fs.existsSync(file)) {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, randomBytes(32).toString("hex"), { mode: 0o600 });
  }
  return fs.readFileSync(file, "utf8").trim();
}

function sameSecret(a: string, b: string): boolean {
  const digest = (s: string) => createHash("sha256").update(s).digest();
  return timingSafeEqual(digest(a), digest(b));
}

function isAllowedOrigin(origin: string): boolean {
  const extra = process.env.TENXGRACE_WS_ALLOWED_ORIGINS?.split(",").map((s) => s.trim()) ?? [];
  if (extra.includes(origin)) return true;
  try {
    return LOOPBACK_HOSTNAMES.has(new URL(origin).hostname);
  } catch {
    return false;
  }
}

/**
 * Gate for WebSocket upgrades. Browsers can't set headers on a WebSocket, so
 * the token travels in the query string. Engines send no Origin; browsers
 * always do, and CORS doesn't apply to WebSockets, so it's checked here.
 */
export function isControlClientAllowed(req: IncomingMessage, token: string): boolean {
  const presented = new URL(req.url ?? "/", "http://localhost").searchParams.get("token");
  if (!presented || !sameSecret(presented, token)) return false;
  return req.headers.origin === undefined || isAllowedOrigin(req.headers.origin);
}
