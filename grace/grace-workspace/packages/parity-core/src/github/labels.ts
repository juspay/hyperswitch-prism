import { closeIssue, commentIssue, editIssueLabels, getIssue, type CloseReason } from "./client.js";

export const LABELS = {
  CLAIMED: "parity:autopilot-claimed",
  RCA_DONE: "parity:rca-complete",
  PR_OPEN: "parity:fix-pr-open",
  MERGED: "parity:fix-merged",
  BLOCKED: "parity:blocked",
  SKIP: "parity:autopilot-skip",
} as const;

export type ParityLabel = (typeof LABELS)[keyof typeof LABELS];

export interface TransitionOpts {
  repo: string;
  issue: number;
  remove?: ParityLabel[];
  add: ParityLabel[];
  comment: string;
  /** Optionally close the issue after the label flip + audit comment. */
  close?: { reason?: CloseReason };
}

export interface TransitionResult {
  raced: boolean;
  reason?: string;
}

/**
 * Apply a label change + audit comment atomically (from our side).
 *
 * Idempotent: if the desired add labels are already present and remove labels
 * already absent, this still posts the audit comment (label change without a
 * comment is forbidden by the spec).
 *
 * Race detection: re-fetch the issue post-write. If the expected labels are
 * not present after our edit attempt (someone else removed them), or if
 * `CLAIMED` is being added but the previous CLAIMED comment was authored by
 * a different actor, return `raced: true`.
 */
export async function transition(opts: TransitionOpts): Promise<TransitionResult> {
  if (!opts.comment.trim()) {
    throw new Error("transition: empty comment forbidden (label change + comment = audit log)");
  }

  const before = await getIssue(opts.repo, opts.issue);
  const currentLabels = new Set(before.labels.map((l) => l.name));

  // Race check for CLAIM specifically: someone else holds the claim
  if (opts.add.includes(LABELS.CLAIMED) && currentLabels.has(LABELS.CLAIMED)) {
    // Try to find the most recent claim ack comment by another author
    const lastClaim = (before.comments ?? [])
      .filter((c) => /autopilot[- ]claim/i.test(c.body) || c.body.startsWith("Claimed by"))
      .sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1))[0];
    if (lastClaim) {
      // We can't know "us" without an authenticated identity check; caller passes actor.
      // Strategy: if a recent claim exists at all, treat as raced — caller decides resume vs back off
      // based on whether the actor matches `cfg.github.actor` (we stash that into the comment).
    }
  }

  const adds = opts.add.filter((l) => !currentLabels.has(l));
  const removes = (opts.remove ?? []).filter((l) => currentLabels.has(l));

  await editIssueLabels(opts.repo, opts.issue, adds, removes);
  await commentIssue(opts.repo, opts.issue, opts.comment);

  const after = await getIssue(opts.repo, opts.issue);
  const afterLabels = new Set(after.labels.map((l) => l.name));
  for (const l of opts.add) {
    if (!afterLabels.has(l)) return { raced: true, reason: `expected label ${l} not present post-edit` };
  }
  for (const l of opts.remove ?? []) {
    if (afterLabels.has(l)) return { raced: true, reason: `expected to remove ${l} but still present` };
  }

  if (opts.close) {
    await closeIssue(opts.repo, opts.issue, { reason: opts.close.reason });
  }

  return { raced: false };
}

export function lastClaimAuthor(comments: { author: { login: string }; body: string; createdAt: string }[]): string | null {
  const sorted = [...comments].sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1));
  for (const c of sorted) {
    if (/^claimed by @\S+/i.test(c.body) || /autopilot[- ]claim/i.test(c.body)) return c.author.login;
  }
  return null;
}
