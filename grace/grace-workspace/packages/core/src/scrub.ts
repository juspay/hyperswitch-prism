import { existsSync, readFileSync } from "node:fs";

/**
 * Fail-closed gate for text GRACE publishes to public GitHub (PR titles/bodies,
 * commit messages, issue bodies, review replies). Callers refuse to publish
 * when `findLeaks` returns anything.
 *
 * Two kinds of hit:
 *   - generic patterns: secrets and production identifiers that are never
 *     legitimate in a public repo (live keys, JWTs, PEM blocks, prod merchant /
 *     payment / request ids, links into the private hyperswitch-cloud tracker);
 *   - needles: exact values the caller knows are private — credential values
 *     from creds.json, identifiers lifted from a private issue.
 */

export interface Leak {
  kind: string;
  match: string;
}

const PATTERNS: [string, RegExp][] = [
  ["private-repo-ref", /hyperswitch-cloud(?:[#/]|\/issues\/)\d*/gi],
  ["jwt", /eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/g],
  ["pem", /-----BEGIN [A-Z ]*(?:PRIVATE KEY|CERTIFICATE)-----/g],
  ["stripe-key", /\b(?:sk|rk)_(?:live|test)_[A-Za-z0-9]{8,}/g],
  ["hs-api-key", /\b(?:pk_)?(?:snd|dev|prd)_[A-Za-z0-9]{32,}\b/g],
  ["bearer", /\bBearer\s+[A-Za-z0-9._~+/=-]{20,}/g],
  ["prod-merchant-id", /\bmerchant_1\d{12}\b/g],
  ["prod-payment-id", /\bpay_[A-Za-z0-9]{20}\b/g],
  ["prod-mca-id", /\bmca_[A-Za-z0-9]{20}\b/g],
  ["request-id", /\b01[0-9a-f]{6}-[0-9a-f]{4}-7[0-9a-f]{3}-[0-9a-f]{4}-[0-9a-f]{12}\b/gi],
  ["occurrence-count", /\b\d+\s*(?:occ|occurrences)\b/gi],
  // "merchant `acmefoods`", "merchant=examplemart" — not a Rust `merchant: Type` field
  ["prod-merchant-label", /\bmerchant(?:=|\s*\*{0,2}`)[A-Za-z0-9_.-]{3,}/gi],
];

// Plain string scans, not regexes: issue text is unbounded and these shapes backtrack badly.
function trimMarks(s: string): string {
  let start = 0;
  let end = s.length;
  while (start < end && "`*'\"".includes(s.charAt(start))) start++;
  while (end > start && "`*'\".,;:)".includes(s.charAt(end - 1))) end--;
  return s.slice(start, end);
}

function titleMerchant(text: string): string | undefined {
  for (const line of text.split("\n")) {
    const at = line.search(/diff:/i);
    if (at < 0) continue;
    const [merchant, connector, ...flow] = line.slice(at + "diff:".length).split("/");
    if (merchant?.trim() && connector && flow.join("/")) return merchant;
  }
  return undefined;
}

/** Identifiers in private issue text that must not follow it into a public PR. */
export function issueNeedles(text: string, allow: string[] = []): string[] {
  const out = new Set<string>();
  const add = (v: string | undefined) => {
    const s = v && trimMarks(v.trim());
    if (s && s.length >= 3) out.add(s);
  };
  // merchant `acmefoods`, merchant=examplemart, org: `ExampleOrg`, org `Example Platform`.
  // A separator or backtick is required so prose ("merchant defined information") yields nothing.
  for (const m of text.matchAll(/\b(?:merchant|org(?:anization)?)(?:[_ ]?(?:id|name))?\s*(?:[=:]\s*\*{0,2}`?|\*{0,2}`)([A-Za-z0-9][A-Za-z0-9_.-]{2,}(?: [A-Z][A-Za-z]+)*)/gi)) add(m[1]);
  // known prod id shapes: bu_xx, tg_example_group, demo_ws_…, merchant_1…, pay_…, mca_…, pro_…
  for (const m of text.matchAll(/\b(?:(?:bu|tg)_[a-z0-9_]+|[a-z]+_ws_[A-Za-z0-9-]+|merchant_\d{10,}|(?:pay|mca|pro)_[A-Za-z0-9]{10,})\b/g)) add(m[0]);
  // validation-service title shape: "[prod] Validation router diff: <merchant> / <connector> / <flow>"
  add(titleMerchant(text));
  const skip = new Set(
    ["merchant", "merchants", "org", "the", "this", "with", "for", "and", "true", "false", "null", ...allow].map((s) => s.toLowerCase()),
  );
  return [...out].filter((s) => !skip.has(s.toLowerCase()));
}

/**
 * Every string credential value in creds.json (≥ 8 chars, not a URL) plus its
 * base64 form — the same set 2.8_pr_run.md `needles()` builds. Missing file →
 * empty list; callers that need creds coverage check the path themselves.
 */
export function credNeedles(credsPath: string | undefined): string[] {
  if (!credsPath || !existsSync(credsPath)) return [];
  const vals = new Set<string>();
  const walk = (v: unknown, key = "") => {
    if (typeof v === "string") {
      if (v.length >= 8 && !/^https?:\/\//.test(v) && !/_comment|auth_type/.test(key)) vals.add(v);
    } else if (v && typeof v === "object") {
      for (const [k, c] of Object.entries(v)) walk(c, k);
    }
  };
  walk(JSON.parse(readFileSync(credsPath, "utf8")));
  return [...vals].flatMap((v) => [v, Buffer.from(v).toString("base64")]);
}

export function findLeaks(text: string, needles: string[] = []): Leak[] {
  const leaks: Leak[] = [];
  for (const [kind, re] of PATTERNS) {
    for (const m of text.matchAll(re)) leaks.push({ kind, match: m[0] });
  }
  const lower = text.toLowerCase();
  for (const n of needles) {
    if (n && lower.includes(n.toLowerCase())) leaks.push({ kind: "needle", match: n });
  }
  return leaks;
}

/** Log-safe summary: kinds and a masked prefix, never the full value. */
export function describeLeaks(leaks: Leak[]): string {
  return leaks.map((l) => `${l.kind}:${l.match.slice(0, 4)}…`).join(", ");
}
