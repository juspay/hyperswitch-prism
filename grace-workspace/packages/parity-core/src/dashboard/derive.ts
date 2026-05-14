import { LABELS } from "../github/labels.js";
import type { Leaf, LeafStatus, Locus } from "../types.js";

export function deriveStatus(leaf: Leaf): LeafStatus {
  const labels = new Set(leaf.labels);
  if (labels.has(LABELS.MERGED) || leaf.linkedPRs.some((p) => p.state === "merged")) return "pr-merged";
  if (labels.has(LABELS.PR_OPEN) || leaf.linkedPRs.some((p) => p.state === "open")) return "pr-open";
  if (labels.has(LABELS.BLOCKED)) return "blocked";
  if (labels.has(LABELS.SKIP)) return "not-applicable";
  return "no-pr";
}

const LOCUS_PATTERNS: { re: RegExp; locus: Exclude<Locus, null> }[] = [
  { re: /^\s*-\s*\[x\]\s*Prism transformer/im, locus: "prism-transformer" },
  { re: /^\s*-\s*\[x\]\s*Hyperswitch UCS bridge/im, locus: "hs-bridge" },
  { re: /^\s*-\s*\[x\]\s*Hyperswitch connector/im, locus: "hs-connector" },
  { re: /^\s*-\s*\[x\]\s*Ambiguous/im, locus: "ambiguous" },
];

/**
 * Pull locus out of the most recent `## Understanding Summary` comment.
 * Returns null if no understanding summary has been posted, or the locus
 * checkbox is missing/multiply-ticked (which the summary parser should
 * have rejected — but we defensively re-check here).
 */
export function deriveLocusFromComments(comments: { body: string; createdAt: string }[]): Locus {
  const summaries = comments
    .filter((c) => c.body.includes("## Understanding Summary"))
    .sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1));
  const latest = summaries[0];
  if (!latest) return null;

  const matched: Locus[] = [];
  for (const p of LOCUS_PATTERNS) {
    if (p.re.test(latest.body)) matched.push(p.locus);
  }
  if (matched.length !== 1) return "ambiguous";
  return matched[0];
}

export interface LabelBackfillFix {
  issue: number;
  remove: string[];
  add: string[];
  reason: string;
}

export function detectLabelBackfills(leaf: Leaf): LabelBackfillFix | null {
  const labels = new Set(leaf.labels);
  const hasMergedPR = leaf.linkedPRs.some((p) => p.state === "merged");

  if (hasMergedPR && !labels.has(LABELS.MERGED)) {
    return {
      issue: leaf.number,
      remove: [LABELS.PR_OPEN, LABELS.CLAIMED].filter((l) => labels.has(l)),
      add: [LABELS.MERGED],
      reason: "PR merged but parity:fix-merged label missing",
    };
  }
  const hasOpenPR = leaf.linkedPRs.some((p) => p.state === "open");
  if (hasOpenPR && !labels.has(LABELS.PR_OPEN) && !labels.has(LABELS.MERGED)) {
    return {
      issue: leaf.number,
      remove: [LABELS.CLAIMED].filter((l) => labels.has(l)),
      add: [LABELS.PR_OPEN],
      reason: "Open PR linked but parity:fix-pr-open label missing",
    };
  }
  return null;
}
