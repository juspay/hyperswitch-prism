export interface LinkedPR {
  repo: string;
  number: number;
  state: "open" | "merged" | "closed";
  mergedAt?: string;
  author?: string;
  url?: string;
}

export interface ParityLeaf {
  number: number;
  title: string;
  body: string;
  labels: string[];
  createdAt: string;
  url: string;
  state: "OPEN" | "CLOSED";
  linkedPRs: LinkedPR[];
  parentTracking: number;
  connector: string;
  flow: string;
}

export type LeafStatus =
  | "no-pr"
  | "pr-open"
  | "pr-merged"
  | "blocked"
  | "closed"
  | "not-applicable";

const LABELS = {
  CLAIMED: "parity:autopilot-claimed",
  RCA_DONE: "parity:rca-complete",
  PR_OPEN: "parity:fix-pr-open",
  MERGED: "parity:fix-merged",
  BLOCKED: "parity:blocked",
  SKIP: "parity:autopilot-skip",
};

export function deriveStatus(leaf: ParityLeaf): LeafStatus {
  const labels = new Set(leaf.labels);
  if (leaf.state === "CLOSED") {
    return leaf.linkedPRs.some((p) => p.state === "merged") ? "pr-merged" : "closed";
  }
  if (labels.has(LABELS.MERGED) || leaf.linkedPRs.some((p) => p.state === "merged")) return "pr-merged";
  if (labels.has(LABELS.PR_OPEN) || leaf.linkedPRs.some((p) => p.state === "open")) return "pr-open";
  if (labels.has(LABELS.BLOCKED)) return "blocked";
  if (labels.has(LABELS.SKIP)) return "not-applicable";
  return "no-pr";
}

export function statusColor(s: LeafStatus): { fg: string; bg: string } {
  switch (s) {
    case "pr-merged": return { fg: "#1f6b2a", bg: "#d8efd0" };
    case "pr-open":   return { fg: "#0a5687", bg: "#cfe4f4" };
    case "no-pr":     return { fg: "#8a6204", bg: "#fbe6c0" };
    case "blocked":   return { fg: "#8a1a1a", bg: "#f8d4d4" };
    case "closed":    return { fg: "#5b21b6", bg: "#e9d5ff" };
    case "not-applicable":
    default:          return { fg: "#666", bg: "#e8e3d6" };
  }
}
