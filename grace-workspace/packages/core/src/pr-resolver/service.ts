import {
  GitHubClient,
  filterTriggeredThreads,
  parseGithubRepo,
} from "./github.js";
import { PrResolverStateManager, type PrMachine } from "./state.js";
import {
  buildThreadView,
  runResolverSession,
  runReviewSummarySession,
} from "./resolver.js";
import { runCargoFixLoop } from "./cargo-loop.js";
import { emitPrResolverEvent } from "./events.js";
import path from "node:path";
import { AsyncLocalStorage } from "node:async_hooks";
import { runClaudeCode } from "../tools/claude-code-runner.js";
import { pickFreePort } from "./port-allocator.js";
import { WorktreePool, type Lease } from "./worktree-pool.js";
import {
  cargoFmt,
  capturePrDiff,
  changedFiles,
  commit,
  ensureWorktree,
  fetchPrHeadSha,
  headSha,
  prepareForPr,
  pushBranch,
  resetToRemote,
  revertAll,
  revertPath,
  stageConnector,
} from "./worktree.js";
import {
  GrpcServerProcess,
  isGrpcurlInstalled,
  runGrpcCommand,
  type GrpcCommandResult,
} from "./grpc-runner.js";
import { renderPrompt } from "./prompts.js";
import {
  loadConnectorCreds,
  orderSteps,
  parseTestPlan,
  runTestPlan,
  type TestPlan,
  type TestStepResult,
} from "./test-plan.js";
import type { CsddConfig, PrResolverConfig } from "../config.js";
import type {
  CycleSummary,
  PRInfo,
  SubTask,
  TriggeredThread,
} from "./types.js";
import type { GrpcTestResultRecord } from "./state.js";

/**
 * Per-PR worker context propagated through AsyncLocalStorage. The
 * worktreePath getter on the service reads from here, so every helper
 * that used to reference `this.worktreePath` automatically sees the
 * leased slot's path when running inside a worker — no explicit param
 * threading needed.
 */
interface WorkerContext {
  lease: Lease;
}
const workerCtx = new AsyncLocalStorage<WorkerContext>();

/**
 * PR Resolver orchestrator. One poll cycle:
 *
 *   1. Fetch open PRs (+ their review threads) for `cfg.githubRepo`.
 *   2. Filter triggered comments (case-insensitive `cfg.trigger` match).
 *   3. Authorize by `authorAssociation` against `cfg.allowedAssociations`.
 *   4. Group by PR; per-PR, group by connector for sub-task scope.
 *   5. Six freshness gates: PR open, baseline-build cache, prepare worktree,
 *      threads still unresolved, baseline build, threads-still-open pre-push.
 *   6. Per sub-task: resolver session → cargo build/clippy fix loop → fmt →
 *      scope check → commit. After all sub-tasks: push, post per-thread reply.
 *
 * Concurrency model: when `cfg.maxConcurrent > 1`, PRs are processed in
 * parallel through a WorktreePool. Each PR acquires a slot for the
 * duration of its cycle; a PR parked in `awaiting_approval` keeps its
 * slot pinned until the reviewer decides. Slot 0 is always the user's
 * primary `cfg.worktreePath` clone; slots 1..N-1 are git worktrees
 * materialised lazily under `cfg.worktreePath`'s sibling pool dir.
 *
 * Other MVP simplifications:
 *   - Questions (vs actionable comments) are marked failed with a polite
 *     note; we don't auto-answer them (drops one Claude session per question).
 *   - No connector AST index — Claude reads the repo directly.
 */
export class PrResolverService {
  private cycle = 0;
  private cancelled = false;
  private readonly state: PrResolverStateManager;
  private readonly github: GitHubClient;
  private readonly owner: string;
  private readonly repo: string;
  /** Per-thread resolution summaries captured during the cycle, replayed when posting replies. */
  private readonly resolveSummaries = new Map<string, string>();
  private lastCycleSummary: CycleSummary | null = null;

  /** Pool of git worktrees — one slot per concurrent PR. Always sized at construction. */
  private readonly pool: WorktreePool;
  /** Currently-running PR workers, keyed by PR number. Promise resolves when the worker exits. */
  private readonly inFlight = new Map<number, Promise<void>>();

  constructor(
    private readonly cfg: PrResolverConfig,
    private readonly rootConfig: CsddConfig
  ) {
    if (!cfg.githubRepo) {
      throw new Error(
        "prResolver.githubRepo is empty — set it in config.yml or BYNE_PR_RESOLVER_GITHUB_REPO"
      );
    }
    const parsed = parseGithubRepo(cfg.githubRepo);
    this.owner = parsed.owner;
    this.repo = parsed.repo;
    this.state = new PrResolverStateManager(cfg.stateFilePath);
    this.github = new GitHubClient(this.owner, this.repo);
    // Pool sized off `maxConcurrent`. Slot 0 is the primary clone; the pool
    // dir is a sibling so a fresh clone doesn't accidentally inherit a
    // bunch of stray worktrees from a prior config.
    const poolDir = path.join(
      path.dirname(path.resolve(cfg.worktreePath)),
      `${path.basename(path.resolve(cfg.worktreePath))}-pool`
    );
    this.pool = new WorktreePool({
      primaryWorktreePath: cfg.worktreePath,
      poolDir,
      maxConcurrent: Math.max(1, cfg.maxConcurrent ?? 1),
    });
  }

  /**
   * Resolve the worktree path that the *current* helper should operate on.
   * Inside a worker (anywhere downstream of `runPr`'s `workerCtx.run`),
   * this is the leased slot's path. Outside any worker — initialise,
   * dashboard control handlers before they look up the machine — it
   * falls back to the primary clone.
   */
  private get worktreePath(): string {
    return workerCtx.getStore()?.lease.worktreePath ?? this.cfg.worktreePath;
  }

  /**
   * Worktree path for a known PR. Used by `approvePr` / `rejectPr` /
   * `requestChanges` to operate on the pinned slot's tree — those entry
   * points run outside of any worker context, so they have to look up
   * the slot explicitly from `machine.workerSlot`.
   */
  private worktreePathForMachine(machine: PrMachine): string {
    if (typeof machine.workerSlot !== "number") return this.cfg.worktreePath;
    const lease = this.pool.findLeaseByPr(machine.prNumber);
    return lease?.worktreePath ?? this.cfg.worktreePath;
  }

  // ─── Public API ─────────────────────────────────────────────────────

  cancel(): void {
    this.cancelled = true;
  }

  isRunning(): boolean {
    // "Running" = any worker is in flight. Replaces the old single
    // `cycleInProgress` boolean which only made sense in the serial model.
    return this.inFlight.size > 0;
  }

  getLastCycleSummary(): CycleSummary | null {
    return this.lastCycleSummary;
  }

  getStateSnapshot() {
    return this.state.snapshot();
  }

  /**
   * Roll up the per-thread resolver summaries captured during this PR's
   * resolve pass into a single Markdown blob the dashboard's approval panel
   * can render. Returns `undefined` when nothing was captured (e.g. the
   * supervisor was restarted between resolve and approval, blowing away the
   * in-memory `resolveSummaries` map).
   */
  private summaryForPr(prNumber: number, threadIds: string[]): string | undefined {
    const blocks: string[] = [];
    const seen = new Set<string>();
    for (const tid of threadIds) {
      const s = this.resolveSummaries.get(tid)?.trim();
      if (!s || seen.has(s)) continue;
      seen.add(s);
      blocks.push(s);
    }
    if (blocks.length === 0) return undefined;
    return blocks.join("\n\n---\n\n").slice(0, 8_000);
  }

  /**
   * Promote an awaiting-approval PR to pushed. Validates that the worktree
   * still matches the captured snapshot and that the remote hasn't moved,
   * then pushes via the no-force `pushBranch` helper and posts replies.
   * Called from the supervisor in response to `pr-resolver:approve`.
   */
  async approvePr(
    prNumber: number,
    note?: string
  ): Promise<{ ok: boolean; error?: string }> {
    const machine = this.state.getPrMachine(prNumber);
    if (!machine) {
      return { ok: false, error: `No machine for PR #${prNumber}` };
    }
    if (machine.status !== "awaiting_approval") {
      return {
        ok: false,
        error: `PR #${prNumber} is in status '${machine.status}', not awaiting approval`,
      };
    }
    // Operate on the pinned slot's worktree, not the primary clone, so we
    // don't push the wrong PR's commits when N>1 workers are in flight.
    const wtPath = this.worktreePathForMachine(machine);
    // Sanity: the worktree should still be at the local SHA we captured.
    const currentLocalSha = await headSha(wtPath);
    if (machine.localSha && currentLocalSha !== machine.localSha) {
      return {
        ok: false,
        error: `Worktree HEAD has moved (expected ${machine.localSha.slice(0, 8)}, got ${currentLocalSha.slice(0, 8)}) — re-run the cycle`,
      };
    }
    // Stale remote: if origin advanced under us, refuse rather than rebase
    // implicitly — the user should see a fresh diff first.
    const currentRemoteSha = await fetchPrHeadSha({
      worktreePath: wtPath,
      owner: this.owner,
      repo: this.repo,
      prNumber,
    });
    if (
      machine.remoteSha &&
      currentRemoteSha &&
      currentRemoteSha !== machine.remoteSha
    ) {
      return {
        ok: false,
        error: `Remote moved since approval was requested (was ${machine.remoteSha.slice(0, 8)}, now ${currentRemoteSha.slice(0, 8)}). Re-poll and review the new diff before approving.`,
      };
    }

    this.state.upsertPrMachine({ prNumber, status: "committing" });
    emitPrResolverEvent("approved", { pr: prNumber, note: note ?? "" });

    const pushResult = await pushBranch(wtPath, machine.branch);
    if (!pushResult.ok) {
      this.state.upsertPrMachine({
        prNumber,
        status: "failed",
        reason: `Push failed: ${pushResult.error ?? "unknown"}`,
      });
      emitPrResolverEvent("pr_failed", {
        pr: prNumber,
        error: pushResult.error,
      });
      // Release the slot so a retry can pick a fresh one.
      this.releasePinnedSlot(prNumber);
      return { ok: false, error: pushResult.error };
    }
    const sha = await headSha(wtPath);
    emitPrResolverEvent("push_done", { pr: prNumber, sha });

    // Reply on each tracked thread. We loop with an index so we can pair
    // each threadId with its trigger comment id (parallel array on the
    // machine), which is what gets written into processed_threads for the
    // per-trigger-comment dedup.
    for (let i = 0; i < machine.threadIds.length; i++) {
      const threadId = machine.threadIds[i]!;
      const triggerCommentId = machine.triggerCommentIds?.[i];
      const summary = this.resolveSummaries.get(threadId) ?? "";
      const detail = summary
        ? summary.slice(0, 500)
        : "Applied fix per your review comment.";
      const body = `**Resolved** in commit \`${sha.slice(0, 8)}\`\n\n${detail}\n\n— *PR Resolver*`;
      const posted = await this.github.postThreadReply(threadId, body);
      if (posted) {
        emitPrResolverEvent("reply_posted", {
          pr: prNumber,
          threadId,
        });
      }
      this.state.markFixed({
        threadId,
        triggerCommentId,
        prNumber,
        commitSha: sha,
        path: "",
        instruction: "",
        resolutionSummary: summary,
      });
    }

    this.state.upsertPrMachine({
      prNumber,
      status: "pushed",
      localSha: sha,
      workerSlot: undefined,
    });
    // Approval flow done — let the slot serve the next PR.
    this.releasePinnedSlot(prNumber);
    return { ok: true };
  }

  /**
   * Unpin and release the worktree slot a PR was holding while parked in
   * awaiting_approval. Idempotent — safe to call even if the PR never had a
   * slot (e.g. it was processed back when maxConcurrent was effectively 1
   * and we didn't persist `workerSlot`). Always called from the approve /
   * reject / requestChanges / retry path, never from inside a worker.
   */
  private releasePinnedSlot(prNumber: number): void {
    const lease = this.pool.findLeaseByPr(prNumber);
    if (!lease) return;
    this.pool.unpin(lease);
    this.pool.release(lease);
  }

  /**
   * Reset a PR back to a pickable state: clear the threads from
   * `processed_threads` so the next poll cycle re-fetches them, remove the
   * machine, reset the worktree to origin (in case we have stale commits
   * sitting around from a half-finished sub-task). Doesn't trigger an
   * immediate poll — the user can hit "Poll Now" if they want one.
   *
   * Allowed states:
   *   - Terminal (`failed | rejected | pushed`) — the normal Retry button.
   *   - Stuck non-terminal (`noticed | preparing | resolving | verifying |
   *     committing`) — only when the resolver isn't actively running. Covers
   *     cases like ENOSPC mid-cycle where the state file couldn't be updated
   *     to "failed", leaving the machine frozen mid-flight. `awaiting_approval`
   *     is intentionally excluded — that's a deliberate hold, not a stuck one.
   */
  async retryPr(
    prNumber: number
  ): Promise<{ ok: boolean; error?: string }> {
    const machine = this.state.getPrMachine(prNumber);
    if (!machine) {
      return { ok: false, error: `No machine for PR #${prNumber}` };
    }
    const terminalStates = new Set(["failed", "rejected", "pushed"]);
    const stuckStates = new Set([
      "noticed",
      "preparing",
      "resolving",
      "verifying",
      "committing",
    ]);
    const isTerminal = terminalStates.has(machine.status);
    // "Stuck" = transient status visible while no worker is actually in
    // flight for this PR — typical ENOSPC mid-cycle. We check inFlight by
    // PR rather than the old global cycleInProgress so other concurrent
    // workers don't make this PR look "live" when its own machine froze.
    const isStuck =
      stuckStates.has(machine.status) && !this.inFlight.has(prNumber);
    if (!isTerminal && !isStuck) {
      if (machine.status === "awaiting_approval") {
        return {
          ok: false,
          error: `PR #${prNumber} is awaiting your approval — use Approve, Reject, or Request changes instead of Retry.`,
        };
      }
      return {
        ok: false,
        error: `Retry blocked: PR #${prNumber} is currently '${machine.status}' and a cycle is in flight. Wait for the cycle to finish, then retry.`,
      };
    }

    // Pull threads back into the unprocessed bucket so pollAndFilter picks
    // them up. retry() is a no-op for thread IDs that aren't currently in
    // processed_threads, so it's safe to call on the full list.
    for (const threadId of machine.threadIds) {
      this.state.retry(threadId);
    }

    // Forget any cached baseline-build failure for this PR. Without this the
    // next cycle keeps short-circuiting on the same SHA and "Retry" doesn't
    // actually retry.
    const cleared = this.state.clearBuildFailure(prNumber);

    // If we left stale local commits behind (rare — most failures revert),
    // reset the slot the PR was using (if any) so the next checkout starts
    // clean. Falling back to the primary clone preserves the legacy
    // single-worktree behaviour for PRs that pre-date `workerSlot`.
    try {
      await resetToRemote(this.worktreePathForMachine(machine), machine.branch);
    } catch {
      /* best-effort — checkout in the next cycle will sort it out */
    }

    // Free the slot regardless of whether the cycle finished cleanly so
    // pool capacity is recovered on Retry.
    this.releasePinnedSlot(prNumber);
    this.state.removePrMachine(prNumber);
    emitPrResolverEvent("pr_retry", {
      pr: prNumber,
      branch: machine.branch,
      threadCount: machine.threadIds.length,
      buildFailureCleared: cleared,
    });
    return { ok: true };
  }

  /**
   * Discard the local commits and post a rejection reply on each thread.
   * The worktree is reset to `origin/<branch>` so the next cycle starts
   * from a clean state. Reason is forwarded into the GitHub reply.
   */
  async rejectPr(
    prNumber: number,
    reason?: string
  ): Promise<{ ok: boolean; error?: string }> {
    const machine = this.state.getPrMachine(prNumber);
    if (!machine) {
      return { ok: false, error: `No machine for PR #${prNumber}` };
    }
    if (machine.status !== "awaiting_approval") {
      return {
        ok: false,
        error: `PR #${prNumber} is in status '${machine.status}', not awaiting approval`,
      };
    }

    const note = (reason ?? "").trim();
    const reset = await resetToRemote(
      this.worktreePathForMachine(machine),
      machine.branch
    );
    if (!reset.ok) {
      return {
        ok: false,
        error: `Reset failed: ${reset.error ?? "unknown"} (worktree may need manual cleanup)`,
      };
    }

    for (let i = 0; i < machine.threadIds.length; i++) {
      const threadId = machine.threadIds[i]!;
      const triggerCommentId = machine.triggerCommentIds?.[i];
      const body = note
        ? `Resolution rejected by reviewer: ${note}\n\n— *PR Resolver*`
        : `Resolution rejected. Feel free to refine your comment and re-trigger.\n\n— *PR Resolver*`;
      await this.github.postThreadReply(threadId, body);
      this.state.markFailed(
        threadId,
        prNumber,
        note ? `Rejected: ${note}` : "Rejected from dashboard",
        triggerCommentId
      );
      emitPrResolverEvent("reply_posted", {
        pr: prNumber,
        threadId,
      });
    }

    this.state.upsertPrMachine({
      prNumber,
      status: "rejected",
      reason: note || "rejected from dashboard",
      workerSlot: undefined,
    });
    emitPrResolverEvent("rejected", { pr: prNumber, reason: note });
    // Slot was pinned through approval; rejection ends that hold.
    this.releasePinnedSlot(prNumber);
    return { ok: true };
  }

  /**
   * Reviewer asked for tweaks while the PR was sitting in awaiting_approval.
   * Discards the proposed commits, stashes the feedback on the machine, and
   * re-queues the threads so the next cycle re-runs the resolve loop with
   * the feedback rendered into the resolve-comment prompt as the overriding
   * instruction. The user can hit "Poll Now" if they don't want to wait for
   * the next scheduled poll.
   */
  async requestChanges(
    prNumber: number,
    feedback: string
  ): Promise<{ ok: boolean; error?: string }> {
    const machine = this.state.getPrMachine(prNumber);
    if (!machine) {
      return { ok: false, error: `No machine for PR #${prNumber}` };
    }
    if (machine.status !== "awaiting_approval") {
      return {
        ok: false,
        error: `PR #${prNumber} is in status '${machine.status}', not awaiting approval`,
      };
    }
    const trimmed = (feedback ?? "").trim();
    if (!trimmed) {
      return {
        ok: false,
        error: "Revision feedback is required — describe what should change.",
      };
    }

    // Discard the proposed commits so the next resolve pass starts from a
    // clean tree pinned to origin/<branch>.
    const reset = await resetToRemote(
      this.worktreePathForMachine(machine),
      machine.branch
    );
    if (!reset.ok) {
      return {
        ok: false,
        error: `Reset failed: ${reset.error ?? "unknown"} (worktree may need manual cleanup)`,
      };
    }

    // Pull the threads back into the pickable bucket — same trick retryPr
    // uses. The next cycle will re-fetch them from GitHub and run the
    // resolve loop again, this time with the feedback in the prompt.
    for (const threadId of machine.threadIds) {
      this.state.retry(threadId);
    }
    // Clear any cached baseline-build failure so we don't short-circuit
    // before the new resolve pass even starts.
    this.state.clearBuildFailure(prNumber);

    this.state.upsertPrMachine({
      prNumber,
      status: "noticed",
      revisionFeedback: trimmed,
      revisionCount: (machine.revisionCount ?? 0) + 1,
      // Wipe the prior diff so the dashboard doesn't keep showing it.
      diffPreview: undefined,
      localSha: undefined,
      reason: undefined,
      // Release the slot — next cycle's processPr will acquire one fresh,
      // possibly a different number. workerSlot is cleared explicitly here
      // even though the persisted machine has it; the next pass overwrites.
      workerSlot: undefined,
    });
    this.releasePinnedSlot(prNumber);

    emitPrResolverEvent("revision_requested", {
      pr: prNumber,
      feedback: trimmed,
      revisionCount: (machine.revisionCount ?? 0) + 1,
    });
    return { ok: true };
  }

  /** Initialise on-disk state + working clone. Idempotent. */
  async initialize(): Promise<void> {
    this.state.load();
    this.state.cleanupOldEntries(30);
    await ensureWorktree({
      worktreePath: this.cfg.worktreePath,
      owner: this.owner,
      repo: this.repo,
    });
    // Re-attach pinned worktree slots for any PR that was sitting in
    // awaiting_approval when the supervisor went down. Without this, the
    // user's pending commits would be orphaned (still on disk in pool/wt-i
    // but not findable via `findLeaseByPr`).
    for (const m of this.state.listPrMachinesByStatus("awaiting_approval")) {
      if (typeof m.workerSlot === "number" && m.workerSlot < this.pool.size()) {
        const lease = this.pool.reattachPinned(
          m.prNumber,
          m.workerSlot,
          m.branch
        );
        if (!lease) {
          // Slot was somehow taken or out of range — clear the persisted
          // assignment so the dashboard can show this PR as recoverable.
          this.state.upsertPrMachine({
            prNumber: m.prNumber,
            workerSlot: undefined,
          });
        }
      }
    }
  }

  /** Long-running poll loop. */
  async runForever(): Promise<void> {
    await this.initialize();
    while (!this.cancelled) {
      try {
        await this.runOnce();
      } catch (err) {
        emitPrResolverEvent("error", {
          phase: "cycle",
          error: err instanceof Error ? err.message : String(err),
        });
      }
      await this.sleepInterruptible(this.cfg.pollInterval * 1000);
    }
  }

  /**
   * Run a single poll cycle. Schedules up to `pool.size()` PR workers in
   * parallel; waits for all newly-dispatched workers to finish before
   * returning so the caller's poll-interval sleep doesn't fire mid-cycle.
   *
   * "Cycle in progress" is no longer a global boolean — we allow re-entry
   * because `runForever` only calls us after the prior cycle resolved, and
   * an outside caller (test, debug) is now expected to coordinate. The
   * per-PR `inFlight` map prevents the same PR from being scheduled twice.
   */
  async runOnce(): Promise<CycleSummary> {
    this.cycle += 1;
    const startedAt = Date.now();
    const summary: CycleSummary = {
      cycle: this.cycle,
      total: 0,
      fixed: 0,
      failed: 0,
      skipped: 0,
      queued: 0,
      startedAt,
      completedAt: 0,
    };
    emitPrResolverEvent("cycle_start", { cycle: this.cycle });

    const dispatchedTasks: Promise<void>[] = [];
    try {
      this.state.load();

      // PRs whose machines are already in awaiting_approval keep their
      // slot pinned and shouldn't be re-queued. Pre-N this used to abort
      // the *whole* cycle; with the pool, other slots can keep working.
      const pendingApproval = new Set(
        this.state
          .listPrMachinesByStatus("awaiting_approval")
          .map((m) => m.prNumber)
      );
      if (pendingApproval.size > 0) {
        emitPrResolverEvent("cycle_skipped_pending_approval", {
          cycle: this.cycle,
          pendingPrs: [...pendingApproval],
        });
      }

      const triggered = await this.pollAndFilter();
      if (triggered.length === 0) {
        emitPrResolverEvent("no_comments", {});
        return summary;
      }
      summary.total = triggered.length;
      this.state.updateLastPoll();

      for (const t of triggered) {
        emitPrResolverEvent("comment_found", {
          pr: t.prNumber,
          path: t.path,
          line: t.line,
          author: t.author,
          instruction: t.instruction.slice(0, 200),
          threadId: t.threadId,
        });
      }

      const byPr = new Map<number, TriggeredThread[]>();
      for (const t of triggered) {
        if (!byPr.has(t.prNumber)) byPr.set(t.prNumber, []);
        byPr.get(t.prNumber)!.push(t);
      }

      // Scheduling: PRs that are pinned in awaiting_approval, or already
      // in flight on another worker, are skipped. The rest try to acquire
      // a worktree slot; when the pool is full, the unscheduled ones get a
      // `pr_queued` event and wait for the next cycle.
      for (const [prNumber, threads] of byPr.entries()) {
        if (this.cancelled) break;
        if (pendingApproval.has(prNumber)) continue;
        if (this.inFlight.has(prNumber)) continue;
        const branch = threads[0]!.prBranch;
        const lease = await this.pool.acquire(prNumber, branch);
        if (!lease) {
          emitPrResolverEvent("pr_queued", { pr: prNumber });
          summary.queued += 1;
          continue;
        }
        const task = this.runPr(prNumber, threads, lease, summary);
        this.inFlight.set(prNumber, task);
        dispatchedTasks.push(task);
      }

      // Wait for everything we *dispatched this cycle* to finish before
      // returning. PRs already in flight from a prior cycle aren't
      // awaited here — they'll surface their own `pr_done`/`pr_failed`
      // events when they complete.
      await Promise.allSettled(dispatchedTasks);
    } finally {
      summary.completedAt = Date.now();
      this.lastCycleSummary = summary;
      emitPrResolverEvent("cycle_end", { ...summary });
    }
    return summary;
  }

  /**
   * One PR worker. Wraps `processPr` in `workerCtx.run` so every helper
   * downstream sees this lease's worktree via `this.worktreePath`. Owns
   * the lease lifecycle: pinned through approval (kept by processPr's
   * awaiting_approval branch), released here on terminal outcomes.
   */
  private async runPr(
    prNumber: number,
    threads: TriggeredThread[],
    lease: Lease,
    summary: CycleSummary
  ): Promise<void> {
    emitPrResolverEvent("pr_start", {
      pr: prNumber,
      threadCount: threads.length,
      slot: lease.slotId,
    });
    try {
      const counts = await workerCtx.run({ lease }, () =>
        this.processPr(prNumber, threads)
      );
      summary.fixed += counts.fixed;
      summary.failed += counts.failed;
      summary.skipped += counts.skipped;
      emitPrResolverEvent("pr_done", { pr: prNumber, ...counts });
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      emitPrResolverEvent("pr_failed", { pr: prNumber, error });
      summary.failed += threads.length;
    } finally {
      // Only release the slot if it wasn't pinned during processPr (the
      // awaiting_approval branch pins; everything else does not). Pinned
      // leases are released by approve/reject/requestChanges/retry.
      if (!lease.pinned) {
        this.pool.release(lease);
      }
      this.inFlight.delete(prNumber);
    }
  }

  // ─── Cycle internals ────────────────────────────────────────────────

  /**
   * Poll GitHub, filter by trigger, authorize by association, dedupe vs
   * processed state, cap by `maxCommentsPerCycle`. Returns the surviving
   * triggered threads ready for processing.
   */
  private async pollAndFilter(): Promise<TriggeredThread[]> {
    const prs = await this.github.fetchOpenPrsWithThreads();
    // New: per-trigger-comment dedup with legacy per-thread fallback.
    // See `getProcessedFilters` for the two-bucket semantics.
    const processed = this.state.getProcessedFilters();
    const all: TriggeredThread[] = [];
    for (const pr of prs) {
      all.push(...filterTriggeredThreads(pr, this.cfg.trigger, processed));
    }
    if (all.length === 0) return [];

    const authorized: TriggeredThread[] = [];
    for (const t of all) {
      if (this.isAuthorized(t)) {
        authorized.push(t);
        continue;
      }
      if (!this.state.isProcessed(t.threadId)) {
        await this.github.postThreadReply(
          t.threadId,
          `@${t.author} You don't have permission to trigger this bot. ` +
            `Only repository members and collaborators can use ${this.cfg.trigger}.\n\n— *PR Resolver*`
        );
        this.state.markFailed(
          t.threadId,
          t.prNumber,
          `Unauthorized: ${t.author} (${t.authorAssociation})`,
          t.commentNodeId
        );
      }
      emitPrResolverEvent("comment_unauthorized", {
        pr: t.prNumber,
        author: t.author,
        association: t.authorAssociation,
        threadId: t.threadId,
      });
    }

    const accepted = authorized.slice(0, this.cfg.maxCommentsPerCycle);

    // 👀 the moment we accept a triggered comment, before we go into the
    // slow path (checkout, build, resolver). GitHub silently dedupes if the
    // bot user already reacted, so re-reacting across cycles is harmless.
    // Done in parallel so a batch of comments doesn't serialize ~N HTTP RTTs.
    await Promise.allSettled(
      accepted
        .filter((t) => t.commentNodeId)
        .map(async (t) => {
          const ok = await this.github.addReaction(t.commentNodeId, "EYES");
          if (ok) {
            emitPrResolverEvent("comment_reacted", {
              pr: t.prNumber,
              threadId: t.threadId,
              author: t.author,
              path: t.path,
              line: t.line,
              commentNodeId: t.commentNodeId,
            });
          }
        })
    );

    return accepted;
  }

  private isAuthorized(t: TriggeredThread): boolean {
    if (this.cfg.blockedUsers.includes(t.author)) return false;
    if (
      this.cfg.allowedUsers.length > 0 &&
      this.cfg.allowedUsers.includes(t.author)
    ) {
      return true;
    }
    return this.cfg.allowedAssociations.includes(t.authorAssociation);
  }

  /** Process one PR end-to-end. Returns thread-level counts. */
  private async processPr(
    prNumber: number,
    threads: TriggeredThread[]
  ): Promise<{ fixed: number; failed: number; skipped: number }> {
    const counts = { fixed: 0, failed: 0, skipped: 0 };
    const branch = threads[0]!.prBranch;
    const machineConnectors = Array.from(
      new Set(threads.map((t) => extractConnector(t.path)))
    );

    this.state.upsertPrMachine({
      prNumber,
      branch,
      status: "preparing",
      threadIds: threads.map((t) => t.threadId),
      triggerCommentIds: threads.map((t) => t.commentNodeId),
      connectors: machineConnectors,
    });

    // GATE 1: PR still open
    const prInfo = await this.github.fetchPrThreads(prNumber);
    if (
      !this.emitGate(prNumber, "PR still open", prInfo?.state === "OPEN", {
        state: prInfo?.state ?? "NOT_FOUND",
      })
    ) {
      this.state.upsertPrMachine({
        prNumber,
        status: "failed",
        reason: `PR not open (state=${prInfo?.state ?? "NOT_FOUND"})`,
      });
      counts.skipped = threads.length;
      return counts;
    }

    // FAST CHECK: skip if we already know this exact HEAD's build is broken.
    const currentSha = await fetchPrHeadSha({
      worktreePath: this.worktreePath,
      owner: this.owner,
      repo: this.repo,
      prNumber,
    });
    if (currentSha && this.state.shouldSkipBuild(prNumber, currentSha)) {
      const failures = this.state.getBuildFailures();
      const stored = failures[String(prNumber)];
      this.emitGate(prNumber, "Baseline build", false, {
        detail: "Build failed previously — waiting for new commits",
        output: stored?.error ?? "",
      });
      this.state.upsertPrMachine({
        prNumber,
        status: "failed",
        reason: "Build failed previously — waiting for new commits on the PR branch",
      });
      counts.skipped = threads.length;
      return counts;
    }

    // GATE 2: prepare working clone for this PR
    const prep = await prepareForPr({
      worktreePath: this.worktreePath,
      prNumber,
    });
    if (!this.emitGate(prNumber, "Checkout branch", prep.ok, { error: prep.error })) {
      this.state.upsertPrMachine({
        prNumber,
        status: "failed",
        reason: `Checkout failed: ${prep.error ?? "unknown"}`,
      });
      counts.skipped = threads.length;
      return counts;
    }
    emitPrResolverEvent("checkout_done", { pr: prNumber, branch });

    // GATE 3: re-verify threads still unresolved
    let stillOpen = threads;
    if (prInfo) {
      const resolvedIds = new Set(
        prInfo.threads.filter((t) => t.isResolved).map((t) => t.id)
      );
      stillOpen = threads.filter((t) => !resolvedIds.has(t.threadId));
    }
    if (
      !this.emitGate(prNumber, "Threads unresolved", stillOpen.length > 0, {
        open: stillOpen.length,
        total: threads.length,
      })
    ) {
      this.state.upsertPrMachine({
        prNumber,
        status: "failed",
        reason: "All threads resolved externally before processing started",
      });
      counts.skipped = threads.length;
      return counts;
    }

    // GATE 4: baseline build (does the PR's HEAD build before we touch it?)
    const baseline = await runCargoBaseline({
      worktreePath: this.worktreePath,
      cargoBuild: this.cfg.cargoBuild,
      timeoutMs: this.cfg.cargoTimeoutMs,
    });
    if (!this.emitGate(prNumber, "Baseline build", baseline.ok, {
      output: baseline.ok ? "" : baseline.output,
    })) {
      const headShaNow = await headSha(this.worktreePath);
      this.state.markBuildFailed({
        prNumber,
        branch,
        headSha: headShaNow,
        error: baseline.output,
        threadIds: stillOpen.map((t) => t.threadId),
      });
      this.state.upsertPrMachine({
        prNumber,
        status: "failed",
        reason: baseline.timedOut
          ? `Baseline build timed out (cargoTimeoutMs=${this.cfg.cargoTimeoutMs}ms). Retry once the dep cache is warm.`
          : "Baseline build of the PR's HEAD failed before any edits",
      });
      counts.skipped = stillOpen.length;
      return counts;
    }

    // Gates 1–4 passed — the worktree is set up and the baseline builds.
    // Move the machine into resolving so the dashboard knows we're actively
    // working. The remoteSha is captured here so a stale check at approval
    // time can detect if the PR moved under us.
    this.state.upsertPrMachine({
      prNumber,
      status: "resolving",
      remoteSha: currentSha || (await headSha(this.worktreePath)),
      threadIds: stillOpen.map((t) => t.threadId),
      triggerCommentIds: stillOpen.map((t) => t.commentNodeId),
    });

    // (👀 reactions already posted by pollAndFilter the moment we accepted
    // the comments — no need to repeat them here after the gates pass.)
    //
    // No more auto-question-reply: previously a regex tried to classify a
    // comment as a "question" and posted a polite "rephrase" reply without
    // user approval — which mis-fired when the trigger was something like
    // "can we resolve this" but the actual review comment was actionable.
    // We now hand every triggered thread to Claude with the full thread
    // context (root + trigger + transcript). If Claude decides nothing is
    // actionable it produces no edits and the sub-task fails with "no
    // changes produced" — the user reviews and rejects with a reason from
    // the dashboard, which posts the only reply that ever leaves the bot.

    // Per-connector sub-tasks
    const byConnector = groupByConnector(stillOpen);
    for (const [connector, connectorThreads] of byConnector) {
      const subTask: SubTask = {
        connector,
        prNumber,
        prBranch: branch,
        threads: connectorThreads,
      };
      const subCounts = await this.processConnector(subTask, prInfo);
      counts.fixed += subCounts.fixed;
      counts.failed += subCounts.failed;
      counts.skipped += subCounts.skipped;
    }

    // After all per-connector sub-tasks: branch on autoApprove.
    if (counts.fixed > 0) {
      const localSha = await headSha(this.worktreePath);
      if (this.cfg.autoApprove) {
        // Auto-mode: push the local commits immediately. State machine still
        // walks through committing → pushed so the timeline reads sensibly.
        this.state.upsertPrMachine({
          prNumber,
          status: "committing",
          localSha,
        });
        await this.pushAndReply(prNumber, branch, stillOpen);
        this.state.upsertPrMachine({
          prNumber,
          status: "pushed",
          localSha: await headSha(this.worktreePath),
        });
      } else {
        // Manual approval: capture the diff for the dashboard, pin this
        // worker's slot so it survives until the reviewer decides, and
        // persist the slot id on the machine so a supervisor restart can
        // reattach. Other pool slots can keep processing other PRs.
        const diff = await capturePrDiff(this.worktreePath, branch);
        const summary = this.summaryForPr(prNumber, stillOpen.map((t) => t.threadId));
        const activeLease = workerCtx.getStore()?.lease;
        if (activeLease) {
          this.pool.pin(activeLease);
        }
        this.state.upsertPrMachine({
          prNumber,
          status: "awaiting_approval",
          localSha,
          diffPreview: diff,
          summary,
          workerSlot: activeLease?.slotId,
          // Reset any prior summary state so the panel shows the spinner
          // immediately instead of stale ready/failed copy from a past cycle.
          reviewSummary: undefined,
          reviewSummaryStatus: "generating",
          reviewSummaryError: undefined,
        });
        emitPrResolverEvent("awaiting_approval", {
          pr: prNumber,
          branch,
          diffChars: diff.length,
          threadCount: stillOpen.length,
        });
        // Stash the still-open threads on the machine so approve/reject
        // can replay replies even after the in-memory list is gone.
        this.state.upsertPrMachine({
          prNumber,
          threadIds: stillOpen.map((t) => t.threadId),
          triggerCommentIds: stillOpen.map((t) => t.commentNodeId),
        });
        // Kick off the reviewer-facing summary in the background. The
        // approval gate is already open — the reviewer can start reading
        // the diff while we generate. Don't await — fire and forget.
        emitPrResolverEvent("review_summary_started", {
          pr: prNumber,
          connectors: machineConnectors,
        });
        void this.generateReviewSummary({
          prNumber,
          connectors: machineConnectors,
          threads: stillOpen,
          diff,
        });
      }
    } else {
      // Nothing succeeded — leave the machine in failed/skipped territory
      // so the dashboard reflects reality.
      this.state.upsertPrMachine({
        prNumber,
        status: "failed",
        reason: counts.failed > 0
          ? "All sub-tasks failed"
          : "All sub-tasks were skipped (e.g. no changes produced)",
      });
    }

    return counts;
  }

  /**
   * Generate a reviewer-facing summary in the background after the approval
   * gate opens. Fire-and-forget — the gate is already actionable, so this
   * latency doesn't block the reviewer.
   *
   * Drops the result silently if the machine's status moved away from
   * `awaiting_approval` while we were running (reviewer approved/rejected
   * mid-summary, or a retry kicked off a new cycle).
   */
  private async generateReviewSummary(input: {
    prNumber: number;
    connectors: string[];
    threads: TriggeredThread[];
    diff: string;
  }): Promise<void> {
    const { prNumber, connectors, threads, diff } = input;
    const connectorLabel = connectors.join(", ") || "(unknown)";
    try {
      // Reuse the same thread view shape the resolve prompt uses, so Claude
      // sees comments the same way it did when generating the changes.
      const threadView = buildThreadView({
        connector: connectorLabel,
        prNumber,
        prBranch: "",
        threads,
      });
      // Diff can be huge — cap to ~24KB so the prompt stays under Claude's
      // tool-friendly window without losing the bulk of the changes.
      const DIFF_CAP = 24_000;
      const cappedDiff =
        diff.length > DIFF_CAP
          ? `... (truncated ${diff.length - DIFF_CAP} chars from head) ...\n` +
            diff.slice(-DIFF_CAP)
          : diff;

      const summary = await runReviewSummarySession({
        prNumber,
        connector: connectorLabel,
        threads: threadView,
        diff: cappedDiff,
        worktreePath: this.worktreePath,
        promptsDir: this.cfg.promptsDir,
        claudeModel: this.rootConfig.claudeCode.model,
      });

      // The reviewer may have approved/rejected/retried while we were
      // generating. Drop the result silently in that case so we don't
      // overwrite a freshly-reset machine.
      const current = this.state.getPrMachine(prNumber);
      if (!current || current.status !== "awaiting_approval") {
        return;
      }
      this.state.upsertPrMachine({
        prNumber,
        reviewSummary: summary,
        reviewSummaryStatus: "ready",
        reviewSummaryError: undefined,
      });
      emitPrResolverEvent("review_summary_generated", {
        pr: prNumber,
        connectors,
        summaryChars: summary.length,
      });
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      const current = this.state.getPrMachine(prNumber);
      if (!current || current.status !== "awaiting_approval") {
        return;
      }
      this.state.upsertPrMachine({
        prNumber,
        reviewSummaryStatus: "failed",
        reviewSummaryError: error,
      });
      emitPrResolverEvent("review_summary_failed", {
        pr: prNumber,
        connectors,
        error,
      });
    }
  }

  /** One connector sub-task: resolver + cargo loop + grpc test + fmt + commit. */
  private async processConnector(
    subTask: SubTask,
    prInfo: PRInfo | null
  ): Promise<{ fixed: number; failed: number; skipped: number }> {
    const counts = { fixed: 0, failed: 0, skipped: 0 };
    const { connector, prNumber, threads } = subTask;
    emitPrResolverEvent("subtask_start", {
      pr: prNumber,
      connector,
      commentCount: threads.length,
    });

    // Pull any reviewer revision feedback off the PR machine — set by
    // requestChanges() on a prior approval cycle. We consume it here and
    // clear it on the machine so a subsequent unrelated re-run doesn't
    // accidentally apply stale feedback.
    const machineBeforeResolve = this.state.getPrMachine(prNumber);
    const revisionFeedback = machineBeforeResolve?.revisionFeedback;
    if (revisionFeedback) {
      this.state.upsertPrMachine({ prNumber, revisionFeedback: undefined });
      emitPrResolverEvent("revision_applying", {
        pr: prNumber,
        connector,
        revisionCount: machineBeforeResolve?.revisionCount ?? 1,
      });
    }

    let claudeSessionId: string;
    try {
      const session = await runResolverSession({
        subTask,
        worktreePath: this.worktreePath,
        promptsDir: this.cfg.promptsDir,
        claudeModel: this.rootConfig.claudeCode.model,
        timeoutMs: this.rootConfig.claudeCode.timeoutMs,
        revisionFeedback,
      });
      claudeSessionId = session.sessionId;
      // Stash per-thread summary so replies can reference what changed.
      for (const t of threads) {
        this.resolveSummaries.set(t.threadId, session.summary);
      }
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      emitPrResolverEvent("subtask_failed", { pr: prNumber, connector, error });
      for (const t of threads) {
        this.state.markFailed(t.threadId, prNumber, error, t.commentNodeId);
      }
      counts.failed = threads.length;
      return counts;
    }

    // Did Claude actually change anything?
    const changed = await changedFiles(this.worktreePath);
    if (changed.length === 0) {
      const reason = `No code changes produced for ${connector} — may already be fixed`;
      emitPrResolverEvent("subtask_gate", {
        pr: prNumber,
        connector,
        gate: "Changes produced",
        passed: false,
        detail: reason,
      });
      for (const t of threads) {
        this.state.markFailed(t.threadId, prNumber, reason, t.commentNodeId);
      }
      counts.skipped = threads.length;
      return counts;
    }

    // Cargo build + clippy fix loop
    const cargo = await runCargoFixLoop({
      subTask,
      worktreePath: this.worktreePath,
      claudeSessionId,
      buildCommand: this.cfg.cargoBuild,
      clippyCommand: this.cfg.cargoClippy,
      maxLoops: this.cfg.maxBuildLoops,
      promptsDir: this.cfg.promptsDir,
      claudeModel: this.rootConfig.claudeCode.model,
      fixTimeoutMs: this.rootConfig.claudeCode.timeoutMs,
      cargoTimeoutMs: this.cfg.cargoTimeoutMs,
    });
    if (!cargo.buildPassed || !cargo.clippyPassed) {
      const reason = `${!cargo.buildPassed ? "Build" : "Clippy"} failed after ${cargo.loopCount} loops`;
      await revertAll(this.worktreePath);
      emitPrResolverEvent("subtask_failed", {
        pr: prNumber,
        connector,
        error: reason,
        output: cargo.errorOutput.slice(-2000),
      });
      for (const t of threads) {
        this.state.markFailed(t.threadId, prNumber, reason, t.commentNodeId);
      }
      counts.failed = threads.length;
      return counts;
    }

    // Phase B: gRPC verification step. With grpcTestEnabled, a missing
    // grpcurl binary or an empty command list now hard-fails (was
    // previously a soft skip) — without verification we don't ship.
    if (this.cfg.grpcTestEnabled) {
      const grpcResult = await this.runGrpcTestStep({
        subTask,
        prInfo,
        sessionId: claudeSessionId,
      });
      if (!grpcResult.passed) {
        const failedCount = grpcResult.stepResults.filter(
          (r) => !r.ok && !r.skipped
        ).length;
        const reason = grpcResult.reason
          ? `gRPC test failed: ${grpcResult.reason}`
          : `gRPC test failed (${failedCount}/${grpcResult.stepResults.length} steps failed)`;
        await revertAll(this.worktreePath);
        emitPrResolverEvent("subtask_failed", {
          pr: prNumber,
          connector,
          error: reason,
        });
        for (const t of threads) {
          this.state.markFailed(t.threadId, prNumber, reason, t.commentNodeId);
        }
        counts.failed = threads.length;
        return counts;
      }
    }

    // Cargo fmt (single shot — no fix loop, just normalize)
    const fmt = await cargoFmt(this.worktreePath);
    emitPrResolverEvent("subtask_gate", {
      pr: prNumber,
      connector,
      gate: "Format",
      passed: fmt.ok,
      detail: fmt.ok ? "PASS" : fmt.output.slice(-200),
    });

    // Scope check: only files mentioning the connector slug should have
    // changed. Revert anything else to keep blast radius tight.
    const after = await changedFiles(this.worktreePath);
    const unexpected = after.filter((f) => !f.includes(connector));
    if (unexpected.length > 0) {
      emitPrResolverEvent("subtask_gate", {
        pr: prNumber,
        connector,
        gate: "Scope",
        passed: false,
        detail: `Reverting ${unexpected.length} out-of-scope file(s)`,
        files: unexpected,
      });
      for (const f of unexpected) {
        await revertPath(this.worktreePath, f);
      }
    }

    // Stage + commit
    const staged = await stageConnector(this.worktreePath, connector);
    if (!staged.ok) {
      const reason = `git add failed: ${staged.error ?? "unknown"}`;
      emitPrResolverEvent("subtask_failed", {
        pr: prNumber,
        connector,
        error: reason,
      });
      counts.failed = threads.length;
      return counts;
    }

    const headline = threads
      .slice(0, 3)
      .map((t) => t.instruction.slice(0, 60))
      .join("; ");
    const message = `fix(${connector}): resolve ${threads.length} review comment(s)\n\n${headline}`;
    const committed = await commit(this.worktreePath, message);
    if (!committed.ok || !committed.sha) {
      const reason = `commit failed: ${committed.error ?? "unknown"}`;
      emitPrResolverEvent("subtask_failed", {
        pr: prNumber,
        connector,
        error: reason,
      });
      counts.failed = threads.length;
      return counts;
    }

    emitPrResolverEvent("subtask_done", {
      pr: prNumber,
      connector,
      sha: committed.sha,
      commentCount: threads.length,
    });
    counts.fixed = threads.length;
    return counts;
  }

  /**
   * Final gates 5–6 + push + per-thread reply. Called after all sub-tasks
   * for a PR have committed.
   */
  private async pushAndReply(
    prNumber: number,
    branch: string,
    threads: TriggeredThread[]
  ): Promise<void> {
    // GATE 5: re-check thread state right before pushing — humans may have
    // resolved or commented while we were working.
    const fresh = await this.github.fetchPrThreads(prNumber);
    if (fresh) {
      const resolvedIds = new Set(
        fresh.threads.filter((t) => t.isResolved).map((t) => t.id)
      );
      const stillOpen = threads.filter((t) => !resolvedIds.has(t.threadId));
      if (stillOpen.length === 0) {
        this.emitGate(prNumber, "Threads still open (pre-push)", false, {
          detail: "all resolved externally",
        });
        return;
      }
      this.emitGate(prNumber, "Threads still open (pre-push)", true, {
        open: stillOpen.length,
      });
    }

    // GATE 6: push
    const push = await pushBranch(this.worktreePath, branch);
    if (!push.ok) {
      emitPrResolverEvent("pr_failed", {
        pr: prNumber,
        error: `push failed: ${push.error ?? "unknown"}`,
      });
      return;
    }
    const sha = await headSha(this.worktreePath);
    emitPrResolverEvent("push_done", { pr: prNumber, sha });

    // Post a reply on every thread we processed in this cycle.
    for (const t of threads) {
      const summary = this.resolveSummaries.get(t.threadId) ?? "";
      const detail = summary
        ? summary.slice(0, 500)
        : `Applied fix for: ${t.instruction.slice(0, 200)}`;
      const body = `**Resolved** in commit \`${sha.slice(0, 8)}\`\n\n${detail}\n\n— *PR Resolver*`;
      const posted = await this.github.postThreadReply(t.threadId, body);
      if (posted) {
        emitPrResolverEvent("reply_posted", {
          pr: prNumber,
          threadId: t.threadId,
        });
      }
      this.state.markFixed({
        threadId: t.threadId,
        triggerCommentId: t.commentNodeId,
        prNumber,
        commitSha: sha,
        path: t.path,
        instruction: t.instruction,
        resolutionSummary: summary,
      });
    }
  }

  // ─── Phase B: gRPC verification ─────────────────────────────────────

  /**
   * Phase B v2: Generate a structured gRPC test plan via a fresh Claude
   * session. Always Claude — no regex extraction from the PR body. Claude
   * itself reads the PR title/body/issue-comments/diff plus the connector's
   * creds block and emits a JSON plan with dependency-ordered steps.
   *
   * Returns `{ plan, reply }` on success, `{ reply, error }` on parse
   * failure (so the dashboard can show the raw reply for diagnostics).
   */
  private async generateTestPlan(
    subTask: SubTask,
    prInfo: PRInfo | null
  ): Promise<{
    plan?: TestPlan;
    reply: string;
    error?: string;
  }> {
    const diff = await capturePrDiff(
      this.worktreePath,
      subTask.prBranch,
      20_000
    );

    // Resolve creds.json. Path precedence: cfg.credsPath (.env) →
    // <projectRoot>/creds.json (the conventional location). We pre-extract
    // just the connector's block so Claude sees structured credentials,
    // not the whole multi-connector file.
    const credsPath =
      this.rootConfig.credsPath ||
      path.join(this.rootConfig.projectRoot, "creds.json");
    const credsResult = loadConnectorCreds(credsPath, subTask.connector);
    if (!credsResult.creds) {
      return {
        reply: "",
        error: credsResult.error,
      };
    }
    const credsBlock = JSON.stringify(credsResult.creds, null, 2);

    // Issue comments — top-level PR conversation. Reviewers sometimes drop
    // grpcurl snippets or test plans here rather than in the structured
    // body, so we surface them to Claude.
    const issueCommentsText = (prInfo?.issueComments ?? [])
      .map(
        (c) =>
          `--- @${c.author} (${c.authorAssociation}) at ${c.createdAt} ---\n${c.body}`
      )
      .join("\n\n");

    const rendered = renderPrompt(
      "grpc-test-plan",
      {
        connector: subTask.connector,
        pr_title: prInfo?.title ?? "",
        pr_body: prInfo?.body ?? "(empty)",
        pr_comments: issueCommentsText || "(no top-level comments)",
        diff: diff || "(no diff yet)",
        grpc_port: String(this.cfg.grpcPort),
        creds_block: credsBlock,
        service_hint:
          "The server exposes `types.PaymentService` over gRPC. Common methods: Authorize, Capture, Reverse, PSync, Refund, RSync, Void. Use the diff to pick which method(s) to exercise.",
      },
      this.cfg.promptsDir
    );

    let reply = "";
    try {
      const { result } = await runClaudeCode<string>({
        skillBody: rendered,
        userPayload: "",
        cwd: this.worktreePath,
        label: `pr-resolver-testplan-${subTask.connector}-pr${subTask.prNumber}`,
        sessionLabel: `pr-resolver-testplan-pr${subTask.prNumber}-${subTask.connector}`,
        timeoutMs: this.rootConfig.claudeCode.timeoutMs,
        rawText: true,
        allowWrite: false,
      });
      reply = result;
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      return { reply: "", error };
    }

    const parsed = parseTestPlan(reply);
    if (!parsed.ok || !parsed.plan) {
      emitPrResolverEvent("grpc_test_plan_parse_error", {
        pr: subTask.prNumber,
        connector: subTask.connector,
        error: parsed.error,
      });
      return { reply, error: parsed.error };
    }
    const ordered = orderSteps(parsed.plan);
    if (!ordered.ok) {
      emitPrResolverEvent("grpc_test_plan_parse_error", {
        pr: subTask.prNumber,
        connector: subTask.connector,
        error: ordered.error,
      });
      return { reply, error: ordered.error };
    }
    emitPrResolverEvent("grpc_test_plan_generated", {
      pr: subTask.prNumber,
      connector: subTask.connector,
      stepCount: parsed.plan.tests.length,
    });
    return { plan: parsed.plan, reply };
  }

  /**
   * Stand up the gRPC server in the worktree, run each grpcurl command, then
   * stop the server. Returns one result per command. Treats grpcurl-missing
   * + empty-command-list as soft skips (we don't want to fail the sub-task
   * for environment issues).
   */
  private async runGrpcTestStep(input: {
    subTask: SubTask;
    prInfo: PRInfo | null;
    sessionId: string;
  }): Promise<{
    passed: boolean;
    stepResults: TestStepResult[];
    reason?: string;
  }> {
    const { subTask, prInfo } = input;

    if (!(await isGrpcurlInstalled())) {
      const reason =
        "grpcurl is not installed on the host. Install it (brew install grpcurl) or disable Run gRPC verification step in the Settings card.";
      emitPrResolverEvent("grpc_test_fail", {
        pr: subTask.prNumber,
        connector: subTask.connector,
        reason,
      });
      return { passed: false, stepResults: [], reason };
    }

    // Single Claude call → structured plan with creds filled in.
    const gen = await this.generateTestPlan(subTask, prInfo);
    this.state.upsertPrMachine({
      prNumber: subTask.prNumber,
      testGenerationReply: gen.reply || undefined,
    });
    if (!gen.plan) {
      const reason = gen.error
        ? `Test plan generation failed: ${gen.error}`
        : "Test plan generation produced no usable JSON. Check the Claude reply on the gRPC test stage panel.";
      emitPrResolverEvent("grpc_test_fail", {
        pr: subTask.prNumber,
        connector: subTask.connector,
        reason,
      });
      return { passed: false, stepResults: [], reason };
    }
    // Persist the plan so the dashboard can render the steps before they
    // start executing.
    this.state.upsertPrMachine({
      prNumber: subTask.prNumber,
      testPlan: gen.plan as unknown as { tests: unknown[] },
    });

    // Allocate two free ports for this worker — one for the gRPC server,
    // one for the metrics/admin server. With N concurrent workers running
    // grpc-server processes against the same host, we cannot rely on the
    // single `cfg.grpcPort` knob; both binds would fail with EADDRINUSE.
    // OS picks both via bind-to-0; we hand them to the spawned process via
    // `CS__SERVER__PORT` / `CS__METRICS_SERVER__PORT` (the ucs_env crate's
    // standard prefix + separator), and use the gRPC port for probe +
    // grpcurl substitution.
    const grpcPort = await pickFreePort();
    const metricsPort = await pickFreePort();

    // Start server.
    emitPrResolverEvent("grpc_server_starting", {
      pr: subTask.prNumber,
      connector: subTask.connector,
      port: grpcPort,
    });
    const server = new GrpcServerProcess({
      worktreePath: this.worktreePath,
      port: grpcPort,
      healthTimeoutMs: this.cfg.grpcServerStartTimeoutMs,
      env: {
        CS__SERVER__PORT: String(grpcPort),
        CS__METRICS_SERVER__PORT: String(metricsPort),
      },
      onStderr: (line) => {
        emitPrResolverEvent("grpc_server_log", {
          pr: subTask.prNumber,
          connector: subTask.connector,
          line,
        });
      },
      onProbe: (attempt, ok) => {
        if (attempt === 1 || ok || attempt % 5 === 0) {
          emitPrResolverEvent("grpc_server_probe", {
            pr: subTask.prNumber,
            connector: subTask.connector,
            attempt,
            ok,
          });
        }
      },
    });
    try {
      await server.start();
    } catch (err) {
      const reason = err instanceof Error ? err.message : String(err);
      emitPrResolverEvent("grpc_test_fail", {
        pr: subTask.prNumber,
        connector: subTask.connector,
        reason: `server start: ${reason}`,
      });
      return {
        passed: false,
        stepResults: [],
        reason,
      };
    }
    emitPrResolverEvent("grpc_server_ready", {
      pr: subTask.prNumber,
      port: grpcPort,
    });

    // Execute the plan with dependency ordering + capture/substitution.
    let stepResults: TestStepResult[] = [];
    let allPassed = false;
    try {
      const run = await runTestPlan({
        plan: gen.plan,
        worktreePath: this.worktreePath,
        port: grpcPort,
        timeoutMs: this.cfg.grpcTestTimeoutMs,
        onStep: (event) => {
          if (event.phase === "start") {
            emitPrResolverEvent("grpc_test_step_start", {
              pr: subTask.prNumber,
              connector: subTask.connector,
              name: event.name,
              depends_on: event.depends_on,
            });
          } else {
            const r = event.result;
            if (r.skipped) {
              emitPrResolverEvent("grpc_test_step_skipped", {
                pr: subTask.prNumber,
                connector: subTask.connector,
                name: r.name,
                reason: r.skipReason,
              });
            } else if (r.ok) {
              emitPrResolverEvent("grpc_test_step_pass", {
                pr: subTask.prNumber,
                connector: subTask.connector,
                name: r.name,
                durationMs: r.durationMs,
              });
            } else {
              emitPrResolverEvent("grpc_test_step_fail", {
                pr: subTask.prNumber,
                connector: subTask.connector,
                name: r.name,
                exitCode: r.exitCode,
                misses: r.expectMisses,
                stderrTail: r.stderr.slice(-400),
              });
            }
          }
          this.state.upsertPrMachine({
            prNumber: subTask.prNumber,
            testStepResults: stepResults,
          });
        },
      });
      stepResults = run.results;
      allPassed = run.ok;
    } finally {
      await server.stop();
      emitPrResolverEvent("grpc_server_stopped", {
        pr: subTask.prNumber,
        connector: subTask.connector,
      });
    }

    this.state.upsertPrMachine({
      prNumber: subTask.prNumber,
      testStepResults: stepResults,
    });
    if (allPassed) {
      emitPrResolverEvent("grpc_test_pass", {
        pr: subTask.prNumber,
        connector: subTask.connector,
        count: stepResults.length,
      });
    } else {
      const failed = stepResults.filter((r) => !r.ok && !r.skipped).length;
      const skipped = stepResults.filter((r) => r.skipped).length;
      emitPrResolverEvent("grpc_test_fail", {
        pr: subTask.prNumber,
        connector: subTask.connector,
        failed,
        skipped,
        total: stepResults.length,
      });
    }
    return {
      passed: allPassed,
      stepResults,
    };
  }

  // ─── Helpers ────────────────────────────────────────────────────────

  private emitGate(
    prNumber: number,
    name: string,
    passed: boolean,
    extra: Record<string, unknown> = {}
  ): boolean {
    emitPrResolverEvent("gate", { pr: prNumber, name, passed, ...extra });
    return passed;
  }

  private async sleepInterruptible(totalMs: number): Promise<void> {
    const tickMs = 1_000;
    const start = Date.now();
    while (!this.cancelled && Date.now() - start < totalMs) {
      await new Promise((r) => setTimeout(r, tickMs));
    }
  }
}

// ─── Module helpers ──────────────────────────────────────────────────

/**
 * Extract connector name from a file path. Mirrors the Python helper —
 * grabs the directory immediately under `connectors/`, dropping any `.rs`
 * extension if the file itself is `<connector>.rs`.
 */
export function extractConnector(filePath: string): string {
  if (filePath.includes("connectors/")) {
    const after = filePath.split("connectors/").pop() ?? "";
    const first = after.split("/")[0] ?? "";
    return first.replace(/\.rs$/, "");
  }
  return "other";
}

/**
 * Detect "is this a question?" — same heuristics as the Python service.
 * Question marks, "why / what / how / is this / should / could …" starts,
 * with override carve-outs for "can you fix", "please change", etc.
 */
export function isQuestion(instruction: string): boolean {
  const text = instruction.trim().toLowerCase();
  if (text.endsWith("?")) return true;
  const questionStarts = [
    "why ",
    "what ",
    "how ",
    "is this",
    "should ",
    "could ",
    "would ",
    "can ",
    "have you",
    "did you",
    "are you",
  ];
  if (questionStarts.some((q) => text.startsWith(q))) {
    const actionable = [
      "can you ",
      "could you ",
      "please ",
      "fix",
      "change",
      "remove",
      "rename",
      "update",
      "add",
      "use ",
    ];
    if (actionable.some((a) => text.includes(a))) return false;
    return true;
  }
  return false;
}

function groupByConnector(
  threads: TriggeredThread[]
): Map<string, TriggeredThread[]> {
  const out = new Map<string, TriggeredThread[]>();
  for (const t of threads) {
    const key = extractConnector(t.path);
    if (!out.has(key)) out.set(key, []);
    out.get(key)!.push(t);
  }
  return out;
}

async function runCargoBaseline(input: {
  worktreePath: string;
  cargoBuild: { command: string; args: string[] };
  timeoutMs: number;
}): Promise<{ ok: boolean; output: string; timedOut: boolean }> {
  const { execa } = await import("execa");
  const result = await execa(input.cargoBuild.command, input.cargoBuild.args, {
    cwd: input.worktreePath,
    reject: false,
    timeout: input.timeoutMs,
    all: true,
  });
  const combined =
    result.all ?? `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
  const timedOut = result.timedOut === true;
  // Label timeouts up front so the cached output reads "timed out" rather
  // than "build failed at <random compile line>".
  const banner = timedOut
    ? `[BASELINE TIMED OUT after ${Math.round(input.timeoutMs / 1000)}s — increase prResolver.cargoTimeoutMs if cold-cache builds need longer]\n\n`
    : "";
  return {
    ok: result.exitCode === 0 && !timedOut,
    output: banner + combined.slice(-10_000),
    timedOut,
  };
}
