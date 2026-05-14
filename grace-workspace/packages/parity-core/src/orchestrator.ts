import { StateManager } from "@byne/core";
import { setTimeout as delay } from "node:timers/promises";
import type { ParityConfig } from "./config.js";
import { loadParityConfig } from "./config.js";
import { walkTree } from "./github/tree.js";
import { writeDashboardFiles } from "./dashboard/renderer.js";
import { deriveStatus, deriveLocusFromComments } from "./dashboard/derive.js";
import { LABELS, lastClaimAuthor, transition } from "./github/labels.js";
import { getIssue } from "./github/client.js";
import { runUnderstand } from "./phases/understand.js";
import { runPlan } from "./phases/plan.js";
import { runExecute } from "./phases/execute.js";
import { verifyLeaf } from "./verify/grpc.js";
import { runHandoff } from "./phases/handoff.js";
import { runSweep } from "./phases/sweep.js";
import { escalate } from "./escalation.js";
import { extendForParity, newHeartbeatId, recordHeartbeat, saveSessionIds, upsertLeaf, getLeafRow } from "./persistence.js";
import type { Leaf } from "./types.js";

export type HeartbeatOutcome =
  | "pr-opened"
  | "verify-failed"
  | "escalated"
  | "no-work"
  | "raced"
  | "error";

export interface HeartbeatResult {
  id: string;
  outcome: HeartbeatOutcome;
  detail?: string;
  pickedLeaf?: number;
}

function decideNextLeaf(leaves: Leaf[], cfg: ParityConfig): Leaf | null {
  const ourClaim = leaves.find((l) => l.labels.includes(LABELS.CLAIMED));
  if (ourClaim) return ourClaim; // resume

  const skipParents = new Set(
    leaves.filter((l) => l.labels.includes(LABELS.SKIP)).map((l) => l.parentTracking),
  );

  const candidates = leaves
    .filter((l) => deriveStatus(l) === "no-pr")
    .filter((l) => !skipParents.has(l.parentTracking))
    .filter((l) => !l.labels.includes(LABELS.SKIP));

  if (candidates.length === 0) return null;

  if (cfg.heartbeat.pickOldestFirst) {
    candidates.sort((a, b) => (a.createdAt < b.createdAt ? -1 : 1));
  }
  return candidates[0];
}

export interface HeartbeatDeps {
  cfg?: ParityConfig;
  state?: StateManager;
  workspaceRoot?: string;
}

export async function runHeartbeat(deps: HeartbeatDeps = {}): Promise<HeartbeatResult> {
  const cfg = deps.cfg ?? loadParityConfig();
  const state = deps.state ?? new StateManager();
  extendForParity(state);
  const workspaceRoot = deps.workspaceRoot ?? process.cwd();

  const id = newHeartbeatId();
  const startedAt = Date.now();
  recordHeartbeat(state, { id, startedAt, outcome: "started" });

  try {
    const leaves = await walkTree(cfg);
    for (const l of leaves) upsertLeaf(state, l, deriveStatus(l));
    await writeDashboardFiles(workspaceRoot, leaves, cfg);
    await runSweep(cfg, leaves);

    const pick = decideNextLeaf(leaves, cfg);
    if (!pick) {
      recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), outcome: "no-work" });
      return { id, outcome: "no-work" };
    }

    const issueRepo = `${cfg.github.owner}/${cfg.github.repo}`;

    // CLAIM (idempotent — skipped if we already hold)
    if (!pick.labels.includes(LABELS.CLAIMED)) {
      const claimResult = await transition({
        repo: issueRepo,
        issue: pick.number,
        add: [LABELS.CLAIMED],
        comment: `Claimed by @${cfg.github.actor}\n\nautopilot-claim heartbeat=${id}`,
      });
      if (claimResult.raced) {
        recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "raced", detail: claimResult.reason });
        return { id, outcome: "raced", pickedLeaf: pick.number, detail: claimResult.reason };
      }
    } else {
      // Confirm we are the claim author
      const fresh = await getIssue(issueRepo, pick.number);
      const author = lastClaimAuthor(fresh.comments ?? []);
      if (author && cfg.github.actor && author !== cfg.github.actor) {
        recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "raced", detail: `claim held by @${author}` });
        return { id, outcome: "raced", pickedLeaf: pick.number };
      }
    }

    const row = getLeafRow(state, pick.number);

    // UNDERSTAND
    const understand = await runUnderstand({ cfg, leaf: pick, sessionId: row?.understand_sid ?? undefined });
    if (understand.sessionId) saveSessionIds(state, pick.number, { understand: understand.sessionId });
    if (!understand.ok || !understand.markdown) {
      await escalate({ cfg, issue: pick.number, step: "understand", ...understand.escalation! });
      recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "escalated", detail: understand.escalation?.blocker });
      return { id, outcome: "escalated", pickedLeaf: pick.number };
    }
    // Post the summary as a comment
    await transition({ repo: issueRepo, issue: pick.number, add: [], comment: understand.markdown });

    // Refresh leaf with the new comment so deriveLocusFromComments works
    const fresh = await getIssue(issueRepo, pick.number);
    const locusFromComments = deriveLocusFromComments(fresh.comments ?? []);
    const effectiveLocus = locusFromComments ?? understand.locus ?? null;

    // PLAN
    const plan = await runPlan({ cfg, leaf: pick, understandMarkdown: understand.markdown, sessionId: row?.plan_sid ?? undefined });
    if (plan.sessionId) saveSessionIds(state, pick.number, { plan: plan.sessionId });
    if (!plan.ok || !plan.markdown) {
      await escalate({ cfg, issue: pick.number, step: "plan", ...plan.escalation! });
      recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "escalated", detail: plan.escalation?.blocker });
      return { id, outcome: "escalated", pickedLeaf: pick.number };
    }
    await transition({ repo: issueRepo, issue: pick.number, add: [LABELS.RCA_DONE], comment: plan.markdown });

    // EXECUTE
    const exec = await runExecute({
      cfg,
      leaf: pick,
      planMarkdown: plan.markdown,
      understoodLocus: effectiveLocus,
      declaredTarget: understand.declaredTarget,
      sessionId: row?.execute_sid ?? undefined,
    });
    if (exec.sessionId) saveSessionIds(state, pick.number, { execute: exec.sessionId });
    if (!exec.ok) {
      await escalate({ cfg, issue: pick.number, step: "execute", ...exec.escalation! });
      recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "escalated", detail: exec.escalation?.blocker });
      return { id, outcome: "escalated", pickedLeaf: pick.number };
    }

    // GRPC_VERIFY
    const verify = await verifyLeaf(cfg, pick);
    await transition({ repo: issueRepo, issue: pick.number, add: [], comment: verify.markdown });
    if (!verify.ok) {
      // No PR. Heartbeat ends. Next heartbeat re-enters at UNDERSTAND because we leave CLAIMED on.
      recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "verify-failed", detail: verify.reason });
      return { id, outcome: "verify-failed", pickedLeaf: pick.number, detail: verify.reason };
    }

    // HANDOFF
    const handoff = await runHandoff({ cfg, leaf: pick, understanding: understand, planMarkdown: plan.markdown, exec, verify });
    if (!handoff.ok) {
      await escalate({ cfg, issue: pick.number, step: "handoff", ...handoff.escalation! });
      recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "escalated", detail: handoff.escalation?.blocker });
      return { id, outcome: "escalated", pickedLeaf: pick.number };
    }

    recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "pr-opened", detail: handoff.prUrl });
    return { id, outcome: "pr-opened", pickedLeaf: pick.number, detail: handoff.prUrl };
  } catch (err) {
    const msg = (err as Error).message || String(err);
    recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), outcome: "error", detail: msg });
    return { id, outcome: "error", detail: msg };
  }
}

export async function runLoop(intervalMs: number, deps: HeartbeatDeps = {}): Promise<never> {
  const cfg = deps.cfg ?? loadParityConfig();
  const state = deps.state ?? new StateManager();
  while (true) {
    const r = await runHeartbeat({ cfg, state, workspaceRoot: deps.workspaceRoot });
    // Brief log to stdout for operators
    // eslint-disable-next-line no-console
    console.log(`[parity] heartbeat ${r.id} → ${r.outcome}${r.pickedLeaf ? ` leaf=#${r.pickedLeaf}` : ""}${r.detail ? ` :: ${r.detail}` : ""}`);
    if (r.outcome === "no-work") {
      // back off harder when there's nothing to do
      await delay(Math.min(intervalMs * 2, 30 * 60 * 1000));
    } else {
      await delay(intervalMs);
    }
  }
}

export async function runDashboardOnly(deps: HeartbeatDeps = {}): Promise<void> {
  const cfg = deps.cfg ?? loadParityConfig();
  const state = deps.state ?? new StateManager();
  extendForParity(state);
  const workspaceRoot = deps.workspaceRoot ?? process.cwd();
  const leaves = await walkTree(cfg);
  for (const l of leaves) upsertLeaf(state, l, deriveStatus(l));
  await writeDashboardFiles(workspaceRoot, leaves, cfg);
}

export async function runSweepOnly(deps: HeartbeatDeps = {}): Promise<void> {
  const cfg = deps.cfg ?? loadParityConfig();
  const leaves = await walkTree(cfg);
  await runSweep(cfg, leaves);
}
