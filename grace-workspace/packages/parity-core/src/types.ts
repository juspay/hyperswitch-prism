export type { RunnerType } from "@10xgrace/core";

export type LeafStatus =
  | "no-pr"
  | "pr-open"
  | "pr-merged"
  | "blocked"
  | "superseded"
  | "not-applicable";

export type Locus =
  | "prism-transformer"
  | "hs-bridge"
  | "hs-connector"
  | "ambiguous"
  | null;

export type LocusTarget = "prism" | "hyperswitch-bridge" | "escalate";

export interface LinkedPR {
  repo: string;
  number: number;
  state: "open" | "merged" | "closed";
  mergedAt?: string;
  author?: string;
  url?: string;
}

export interface Leaf {
  number: number;
  title: string;
  body: string;
  labels: string[];
  createdAt: string;
  url: string;
  linkedPRs: LinkedPR[];
  parentTracking: number;
  connector: string;
  flow: string;
}

export interface UnderstandingResult {
  ok: boolean;
  markdown?: string;
  locus?: Locus;
  confidence?: "HIGH" | "MEDIUM" | "LOW";
  declaredTarget?: "prism" | "hyperswitch-bridge";
  escalation?: { blocker: string; tried: string; question: string };
}

export interface PlanResult {
  ok: boolean;
  markdown?: string;
  files?: string[];
  escalation?: { blocker: string; tried: string; question: string };
}

export interface ExecuteResult {
  ok: boolean;
  target: LocusTarget;
  repoRoot: string;
  branch: string;
  changedFiles: string[];
  buildTail: string;
  clippyTail: string;
  testTail: string;
  escalation?: { blocker: string; tried: string; question: string };
}

export interface VerifyResult {
  ok: boolean;
  markdown: string;
  reason?: string;
  fieldDiffs?: { path: string; expected: unknown; observed: unknown; match: boolean }[];
}
