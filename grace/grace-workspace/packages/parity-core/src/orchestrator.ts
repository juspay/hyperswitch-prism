import { StateManager, type RunnerType } from "@10xgrace/core";
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
import { emit } from "./progress.js";
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
  if (ourClaim) return ourClaim;

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
  /** Run autopilot for this specific leaf instead of FIFO pick. */
  leafNumber?: number;
  /** Skip CLAIM + HANDOFF + comment posts + parity:blocked transitions. Agents + cargo + grpc verify still run. */
  dryRun?: boolean;
  /** Override parityAutopilot.llm.runner for this heartbeat (e.g. force claude-code regardless of config). */
  runnerOverride?: RunnerType;
  /** Override parityAutopilot.bridgeWritePath for this heartbeat (CLI --bridge-path). */
  bridgePathOverride?: string;
  /** Override parityAutopilot.oracleReadOnlyPath for this heartbeat (CLI --oracle-path). */
  oraclePathOverride?: string;
}

export async function runHeartbeat(deps: HeartbeatDeps = {}): Promise<HeartbeatResult> {
  const baseCfg = deps.cfg ?? loadParityConfig();
  // Build an effective cfg applying any per-heartbeat overrides.
  const cfg: ParityConfig = {
    ...baseCfg,
    llm: deps.runnerOverride ? { ...baseCfg.llm, runner: deps.runnerOverride } : baseCfg.llm,
    bridgeWritePath: deps.bridgePathOverride ?? baseCfg.bridgeWritePath,
    oracleReadOnlyPath: deps.oraclePathOverride ?? baseCfg.oracleReadOnlyPath,
  };
  const state = deps.state ?? new StateManager();
  extendForParity(state);
  const workspaceRoot = deps.workspaceRoot ?? process.cwd();
  const dryRun = deps.dryRun ?? false;
  const issueRepo = `${cfg.github.owner}/${cfg.github.repo}`;

  const id = newHeartbeatId();
  const startedAt = Date.now();
  recordHeartbeat(state, { id, startedAt, outcome: "started" });

  try {
    emit({ phase: "discover", status: "start" });
    const leaves = await walkTree(cfg);
    emit({ phase: "discover", status: "ok", leafCount: leaves.length });

    for (const l of leaves) upsertLeaf(state, l, deriveStatus(l));
    await writeDashboardFiles(workspaceRoot, leaves, cfg);
    emit({ phase: "refresh", status: "ok" });

    // Skip sweep in dry-run — its job is to write labels and post reminders.
    if (!dryRun) {
      await runSweep(cfg, leaves);
      emit({ phase: "sweep", status: "ok" });
    }

    let pick: Leaf | null;
    if (deps.leafNumber !== undefined) {
      pick = leaves.find((l) => l.number === deps.leafNumber) ?? null;
      if (!pick) {
        emit({ phase: "decide", status: "fail", reason: `leaf #${deps.leafNumber} not in tree` });
        recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), outcome: "error", detail: `leaf #${deps.leafNumber} not in tree` });
        emit({ phase: "done", outcome: "error", detail: `leaf #${deps.leafNumber} not in tree` });
        return { id, outcome: "error", detail: `leaf #${deps.leafNumber} not in tree` };
      }
    } else {
      pick = decideNextLeaf(leaves, cfg);
    }

    if (!pick) {
      emit({ phase: "decide", status: "ok" });
      emit({ phase: "done", outcome: "no-work" });
      recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), outcome: "no-work" });
      return { id, outcome: "no-work" };
    }
    emit({ phase: "decide", status: "ok", picked: pick.number });

    // CLAIM
    if (dryRun) {
      emit({ phase: "claim", status: "skipped-dry-run", detail: `would claim #${pick.number}` });
      // eslint-disable-next-line no-console
      console.log(`[dry-run] skipping claim of #${pick.number} (no label, no comment)`);
    } else if (!pick.labels.includes(LABELS.CLAIMED)) {
      emit({ phase: "claim", status: "start" });
      const claimResult = await transition({
        repo: issueRepo,
        issue: pick.number,
        add: [LABELS.CLAIMED],
        comment: `Claimed by @${cfg.github.actor}\n\nautopilot-claim heartbeat=${id}`,
      });
      if (claimResult.raced) {
        emit({ phase: "claim", status: "raced", detail: claimResult.reason });
        emit({ phase: "done", outcome: "raced", detail: claimResult.reason, pickedLeaf: pick.number });
        recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "raced", detail: claimResult.reason });
        return { id, outcome: "raced", pickedLeaf: pick.number, detail: claimResult.reason };
      }
      emit({ phase: "claim", status: "ok" });
    } else {
      const fresh = await getIssue(issueRepo, pick.number);
      const author = lastClaimAuthor(fresh.comments ?? []);
      if (author && cfg.github.actor && author !== cfg.github.actor) {
        emit({ phase: "claim", status: "raced", detail: `claim held by @${author}` });
        emit({ phase: "done", outcome: "raced", detail: `claim held by @${author}`, pickedLeaf: pick.number });
        recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "raced", detail: `claim held by @${author}` });
        return { id, outcome: "raced", pickedLeaf: pick.number };
      }
      emit({ phase: "claim", status: "ok", detail: "already held" });
    }

    const row = getLeafRow(state, pick.number);

    // UNDERSTAND
    emit({ phase: "understand", status: "start" });
    const understand = await runUnderstand({ cfg, leaf: pick, sessionId: row?.understand_sid ?? undefined });
    if (understand.sessionId) saveSessionIds(state, pick.number, { understand: understand.sessionId });
    if (!understand.ok || !understand.markdown) {
      emit({ phase: "understand", status: "fail" });
      await escalate({ cfg, issue: pick.number, step: "understand", ...understand.escalation!, dryRun });
      emit({ phase: "done", outcome: "escalated", detail: understand.escalation?.blocker, pickedLeaf: pick.number });
      recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "escalated", detail: understand.escalation?.blocker });
      return { id, outcome: "escalated", pickedLeaf: pick.number };
    }
    emit({ phase: "understand", status: "ok", confidence: understand.confidence, locus: understand.locus ?? undefined, markdown: understand.markdown });

    let effectiveLocus = understand.locus ?? null;
    if (dryRun) {
      // eslint-disable-next-line no-console
      console.log(`[dry-run] not posting understanding summary to #${pick.number}`);
    } else {
      await transition({ repo: issueRepo, issue: pick.number, add: [], comment: understand.markdown });
      const fresh = await getIssue(issueRepo, pick.number);
      effectiveLocus = deriveLocusFromComments(fresh.comments ?? []) ?? effectiveLocus;
    }

    // PLAN
    emit({ phase: "plan", status: "start" });
    const plan = await runPlan({ cfg, leaf: pick, understandMarkdown: understand.markdown, sessionId: row?.plan_sid ?? undefined });
    if (plan.sessionId) saveSessionIds(state, pick.number, { plan: plan.sessionId });
    if (!plan.ok || !plan.markdown) {
      emit({ phase: "plan", status: "fail" });
      await escalate({ cfg, issue: pick.number, step: "plan", ...plan.escalation!, dryRun });
      emit({ phase: "done", outcome: "escalated", detail: plan.escalation?.blocker, pickedLeaf: pick.number });
      recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "escalated", detail: plan.escalation?.blocker });
      return { id, outcome: "escalated", pickedLeaf: pick.number };
    }
    emit({ phase: "plan", status: "ok", markdown: plan.markdown });

    if (dryRun) {
      // eslint-disable-next-line no-console
      console.log(`[dry-run] not posting implementation plan to #${pick.number} (no parity:rca-complete label)`);
    } else {
      await transition({ repo: issueRepo, issue: pick.number, add: [LABELS.RCA_DONE], comment: plan.markdown });
    }

    // EXECUTE
    emit({ phase: "execute", status: "start" });
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
      emit({ phase: "execute", status: "fail", target: exec.target, branch: exec.branch, tail: exec.buildTail });
      await escalate({ cfg, issue: pick.number, step: "execute", ...exec.escalation!, dryRun });
      emit({ phase: "done", outcome: "escalated", detail: exec.escalation?.blocker, pickedLeaf: pick.number });
      recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "escalated", detail: exec.escalation?.blocker });
      return { id, outcome: "escalated", pickedLeaf: pick.number };
    }
    emit({ phase: "execute", status: "ok", target: exec.target, branch: exec.branch });

    // GRPC_VERIFY (short-circuited for hs-bridge — see verify/grpc.ts)
    emit({ phase: "verify", status: "start" });
    const verify = await verifyLeaf(cfg, pick, exec.target);
    if (!dryRun) {
      await transition({ repo: issueRepo, issue: pick.number, add: [], comment: verify.markdown });
    } else {
      // eslint-disable-next-line no-console
      console.log(`[dry-run] not posting grpc verification block to #${pick.number}`);
    }
    if (!verify.ok) {
      emit({ phase: "verify", status: "fail", markdown: verify.markdown, reason: verify.reason });
      emit({ phase: "done", outcome: "verify-failed", detail: verify.reason, pickedLeaf: pick.number });
      recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "verify-failed", detail: verify.reason });
      return { id, outcome: "verify-failed", pickedLeaf: pick.number, detail: verify.reason };
    }
    emit({ phase: "verify", status: "ok", markdown: verify.markdown });

    // HANDOFF
    emit({ phase: "handoff", status: dryRun ? "skipped-dry-run" : "start" });
    const handoff = await runHandoff({ cfg, leaf: pick, understanding: understand, planMarkdown: plan.markdown, exec, verify, dryRun });
    if (!handoff.ok) {
      emit({ phase: "handoff", status: "fail" });
      await escalate({ cfg, issue: pick.number, step: "handoff", ...handoff.escalation!, dryRun });
      emit({ phase: "done", outcome: "escalated", detail: handoff.escalation?.blocker, pickedLeaf: pick.number });
      recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome: "escalated", detail: handoff.escalation?.blocker });
      return { id, outcome: "escalated", pickedLeaf: pick.number };
    }
    emit({ phase: "handoff", status: dryRun ? "skipped-dry-run" : "ok", prUrl: handoff.prUrl });

    const outcome: HeartbeatOutcome = "pr-opened";
    emit({ phase: "done", outcome, detail: handoff.prUrl, pickedLeaf: pick.number });
    recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), pickedLeaf: pick.number, outcome, detail: handoff.prUrl });
    return { id, outcome, pickedLeaf: pick.number, detail: handoff.prUrl };
  } catch (err) {
    const msg = (err as Error).message || String(err);
    emit({ phase: "done", outcome: "error", detail: msg });
    recordHeartbeat(state, { id, startedAt, completedAt: Date.now(), outcome: "error", detail: msg });
    return { id, outcome: "error", detail: msg };
  }
}

export async function runLoop(intervalMs: number, deps: HeartbeatDeps = {}): Promise<never> {
  const cfg = deps.cfg ?? loadParityConfig();
  const state = deps.state ?? new StateManager();
  while (true) {
    const r = await runHeartbeat({ ...deps, cfg, state });
    // eslint-disable-next-line no-console
    console.log(`[parity] heartbeat ${r.id} → ${r.outcome}${r.pickedLeaf ? ` leaf=#${r.pickedLeaf}` : ""}${r.detail ? ` :: ${r.detail}` : ""}`);
    if (r.outcome === "no-work") {
      await delay(Math.min(intervalMs * 2, 30 * 60 * 1000));
    } else {
      await delay(intervalMs);
    }
  }
}

export async function runDashboardOnly(
  deps: HeartbeatDeps = {},
  opts: { force?: boolean } = {},
): Promise<void> {
  const cfg = deps.cfg ?? loadParityConfig();
  const state = deps.state ?? new StateManager();
  extendForParity(state);
  const workspaceRoot = deps.workspaceRoot ?? process.cwd();
  const leaves = await walkTree(cfg, { force: opts.force });
  for (const l of leaves) upsertLeaf(state, l, deriveStatus(l));
  await writeDashboardFiles(workspaceRoot, leaves, cfg);
}

export async function runSweepOnly(deps: HeartbeatDeps = {}): Promise<void> {
  const cfg = deps.cfg ?? loadParityConfig();
  const leaves = await walkTree(cfg);
  await runSweep(cfg, leaves);
}
