import { createHash } from "node:crypto";

/**
 * Phase 15: deterministic Claude session ids derived from
 * `(connector, flow/payment-method, phase)`.
 *
 * The runner's `--session-id` flag wants a UUID-shaped string (8-4-4-4-12
 * hex). We produce one by SHA-1ing a namespaced friendly name and slicing
 * the hex into UUID form. Same friendly name → same id, always; different
 * name → different id (cryptographically). This is structurally similar to
 * UUIDv5 but doesn't bother setting the version/variant bits — claude CLI
 * doesn't enforce them, only the shape.
 *
 * The friendly name (`stripe-card3ds-implementation`) is what we log
 * alongside the uuid in the runner so engine output is grep-able by name.
 *
 * Cross-run implication: two pipeline runs of the same `(connector, flow,
 * phase)` produce identical ids, so the second run's runAI call issues
 * `--resume <same-uuid>` and picks up the prior conversation. This is the
 * intentional behaviour — connector implementation effort is cumulative;
 * the L2/L3/codegen Claudes remember what was tried before. To get a fresh
 * conversation, the user must explicitly delete the jsonl (e.g. via
 * `10xgrace sessions prune` or a future targeted reset command).
 */

const TENXGRACE_NAMESPACE = "10xgrace-grace-pipeline";

/** Lowercase + strip every non-alphanumeric character (no replacement, no
 *  underscores). Lossy on purpose so display variants ("Card 3DS" /
 *  "card-3ds" / "Card3DS") all collapse to the same key "card3ds". The
 *  empty-string guard handles the edge case where the input is entirely
 *  punctuation (extremely unlikely in practice). */
function norm(s: string | undefined | null): string {
  const stripped = (s ?? "unknown").toLowerCase().replace(/[^a-z0-9]+/g, "");
  return stripped.length > 0 ? stripped : "unknown";
}

/**
 * Stable, human-readable identifier for one `(connector, flow, phase)`
 * tuple. Logged in engine output so `grep stripe-card3ds-implementation`
 * pulls every relevant spawn line.
 *
 * Only connector and flow are normalized — they're user-supplied display
 * strings ("Stripe", "Card 3DS"). The phase tag is a code-controlled
 * internal identifier already in canonical form ("l2planning-links",
 * "grpctest") and is passed through verbatim so its internal hyphens are
 * preserved as readable sub-segment separators.
 */
export function friendlySessionName(
  connector: string | undefined,
  flow: string | undefined,
  phase: string
): string {
  return `${norm(connector)}-${norm(flow)}-${phase}`;
}

/**
 * Deterministic UUID-shaped session id. Pass to `runAI` as
 * `preferredSessionId` on first call; the runner forwards it to
 * `claude --session-id <derived>` instead of generating a random uuid.
 *
 * Same friendly name → same returned uuid, always.
 */
export function deriveClaudeSessionId(
  connector: string | undefined,
  flow: string | undefined,
  phase: string
): string {
  const fname = friendlySessionName(connector, flow, phase);
  const hex = createHash("sha1")
    .update(`${TENXGRACE_NAMESPACE}/${fname}`)
    .digest("hex");
  return [
    hex.slice(0, 8),
    hex.slice(8, 12),
    hex.slice(12, 16),
    hex.slice(16, 20),
    hex.slice(20, 32),
  ].join("-");
}
