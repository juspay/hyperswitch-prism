import { execa } from "execa";
import fs from "node:fs";
import path from "node:path";

/**
 * Pool of git worktrees. Slot 0 is special — it points at `cfg.worktreePath`,
 * the user's primary clone, so a 1-slot pool behaves identically to the
 * legacy single-worktree mode (no `git worktree add`, no extra disk).
 *
 * Slots 1..N-1 live under `poolDir` and are created lazily via
 * `git worktree add` against the primary clone's `.git/` object store.
 * Empty worktrees share the dependency object DB with the primary, so the
 * incremental disk cost per slot is just the checked-out source tree
 * (~few MB) plus whatever `target/` accumulates as cargo builds.
 *
 * The pool does NOT own `target/` seeding — that's deferred to a v2.
 * The first cycle on each new slot will pay a cold cargo build cost.
 *
 * Concurrency model: every public method is synchronous *as far as JS is
 * concerned*. The async parts (git worktree add, cleanup) are awaited
 * sequentially within each method body, but multiple callers can race
 * `acquire` — the slot-search is done atomically before any await, so
 * two waiters never get the same slot.
 */

export interface PoolOptions {
  /** Primary clone path — slot 0 IS this directory; no extra worktree. */
  primaryWorktreePath: string;
  /** Where slots 1..N-1 live. Each slot is a sibling subdirectory. */
  poolDir: string;
  /** Max concurrent slots. ≥ 1. */
  maxConcurrent: number;
}

export interface Lease {
  /** 0..maxConcurrent-1. Slot 0 is the primary worktree. */
  slotId: number;
  /** Absolute path to this slot's working tree. */
  worktreePath: string;
  /** PR currently held by this lease. */
  prNumber: number;
  /** True while the PR is parked in awaiting_approval — slot can't be reassigned. */
  pinned: boolean;
}

interface SlotState {
  slotId: number;
  worktreePath: string;
  /** null when free. */
  lease: Lease | null;
  /** True once `git worktree add` has been run (or true forever for slot 0). */
  initialised: boolean;
}

export class WorktreePool {
  private readonly slots: SlotState[] = [];

  constructor(private readonly opts: PoolOptions) {
    if (opts.maxConcurrent < 1) {
      throw new Error(
        `WorktreePool: maxConcurrent must be >= 1, got ${opts.maxConcurrent}`
      );
    }
    // Slot 0 — primary clone, always initialised.
    this.slots.push({
      slotId: 0,
      worktreePath: opts.primaryWorktreePath,
      lease: null,
      initialised: true,
    });
    // Slots 1..N-1 — lazy, materialised under poolDir.
    for (let i = 1; i < opts.maxConcurrent; i++) {
      this.slots.push({
        slotId: i,
        worktreePath: path.join(opts.poolDir, `wt-${i}`),
        lease: null,
        initialised: false,
      });
    }
  }

  /**
   * Attempt to acquire a free slot for `prNumber` on `branch`. Returns
   * `null` if every slot is leased — caller decides whether to retry
   * later, skip the PR, or queue. The branch arg is used to bootstrap
   * the worktree on a freshly-materialised slot; existing slots get a
   * `git checkout` to the new branch later in `prepareForPr`.
   */
  async acquire(prNumber: number, branch: string): Promise<Lease | null> {
    const slot = this.slots.find((s) => s.lease === null);
    if (!slot) return null;

    // Mark held *before* any await so a concurrent acquire can't take it.
    const lease: Lease = {
      slotId: slot.slotId,
      worktreePath: slot.worktreePath,
      prNumber,
      pinned: false,
    };
    slot.lease = lease;

    if (!slot.initialised) {
      try {
        await this.materialiseSlot(slot, branch);
        slot.initialised = true;
      } catch (err) {
        slot.lease = null;
        throw err;
      }
    }
    return lease;
  }

  /**
   * Release a non-pinned lease. Called when the cycle ends without parking
   * the PR in awaiting_approval (success / failed / rejected / skipped).
   * Pinned leases must be `unpin`'d first.
   */
  release(lease: Lease): void {
    const slot = this.slots[lease.slotId];
    if (!slot) {
      throw new Error(`WorktreePool.release: unknown slotId ${lease.slotId}`);
    }
    if (slot.lease !== lease) {
      // Defensive: someone else's release call. Refuse silently.
      return;
    }
    if (lease.pinned) {
      throw new Error(
        `WorktreePool.release: cannot release a pinned lease (PR #${lease.prNumber}). ` +
          `Call unpin first.`
      );
    }
    slot.lease = null;
  }

  /** Mark a lease as pinned — its slot stays leased through approval. */
  pin(lease: Lease): void {
    lease.pinned = true;
  }

  /** Reverse `pin`. After this, `release` is legal. */
  unpin(lease: Lease): void {
    lease.pinned = false;
  }

  /**
   * Re-attach a PrMachine to its prior slot during supervisor boot. The
   * caller has read `machine.workerSlot` from state.json and wants to
   * reserve that slot for the PR (which is presumably in
   * awaiting_approval). Skips the `git worktree add` because the slot is
   * already materialised on disk.
   *
   * Returns `null` if the slot is out of range for the current pool size
   * (e.g., maxConcurrent was reduced) — caller should treat the machine
   * as orphaned and reset it.
   */
  reattachPinned(prNumber: number, slotId: number, branch: string): Lease | null {
    const slot = this.slots[slotId];
    if (!slot) return null;
    if (slot.lease !== null) {
      // Slot is already held by something — refuse rather than overwrite.
      return null;
    }
    const lease: Lease = {
      slotId,
      worktreePath: slot.worktreePath,
      prNumber,
      pinned: true,
    };
    slot.lease = lease;
    // Assume initialised on disk — if not, the next operation against the
    // worktree will surface the error and the caller can recover.
    slot.initialised = true;
    return lease;
  }

  /** Count of slots currently free. Useful for logs and scheduling caps. */
  freeSlotCount(): number {
    return this.slots.filter((s) => s.lease === null).length;
  }

  /**
   * Look up the lease currently held by `prNumber`, if any. Used by the
   * dashboard-triggered control paths (approve/reject/requestChanges) to
   * route operations to the pinned slot's worktree instead of guessing.
   */
  findLeaseByPr(prNumber: number): Lease | null {
    for (const slot of this.slots) {
      if (slot.lease && slot.lease.prNumber === prNumber) {
        return slot.lease;
      }
    }
    return null;
  }

  /** Total slots (= maxConcurrent). */
  size(): number {
    return this.slots.length;
  }

  /** All currently-leased PRs. Useful for snapshots / debug. */
  leasedPrs(): Array<{ slotId: number; prNumber: number; pinned: boolean }> {
    return this.slots
      .filter((s): s is SlotState & { lease: Lease } => s.lease !== null)
      .map((s) => ({
        slotId: s.slotId,
        prNumber: s.lease.prNumber,
        pinned: s.lease.pinned,
      }));
  }

  /**
   * `git worktree add` the slot under poolDir. The branch is just a
   * starting point — the resolver's own `prepareForPr` will fetch the PR
   * and `checkout -B`, so we don't have to perfectly match here.
   */
  private async materialiseSlot(slot: SlotState, branch: string): Promise<void> {
    fs.mkdirSync(this.opts.poolDir, { recursive: true });
    // `git worktree add -f` lets us reuse a path that already exists on
    // disk from a prior process (e.g., orphan from a crashed supervisor).
    // We pair it with `--detach` so we don't fight git over branch
    // ownership — the resolver's `checkout -B` will sort the head out.
    const res = await execa(
      "git",
      ["worktree", "add", "-f", "--detach", slot.worktreePath, "HEAD"],
      { cwd: this.opts.primaryWorktreePath, reject: false, timeout: 120_000 }
    );
    if (res.exitCode !== 0) {
      // If the path is already a registered worktree, that's actually fine
      // — git will tell us, and the existing tree is reusable. Detect and
      // continue.
      const stderr = (res.stderr ?? "").toLowerCase();
      if (
        stderr.includes("already exists") ||
        stderr.includes("already registered")
      ) {
        return;
      }
      throw new Error(
        `git worktree add ${slot.worktreePath} failed (rc=${res.exitCode}): ${res.stderr || res.stdout}`
      );
    }
    void branch;
  }
}
