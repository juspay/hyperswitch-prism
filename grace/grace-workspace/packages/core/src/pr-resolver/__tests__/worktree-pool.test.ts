import { describe, expect, it, beforeEach, afterEach, vi } from "vitest";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execa } from "execa";
import { WorktreePool } from "../worktree-pool.js";

/**
 * Tests for the pool's slot-management logic. The git worktree spawn is
 * intentionally exercised by mocking execa — these tests are about the
 * pool's bookkeeping, not git's behaviour.
 */

vi.mock("execa", () => ({
  execa: vi.fn(async () => ({
    exitCode: 0,
    stdout: "",
    stderr: "",
  })),
}));

function tmpDir(name: string): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), `pool-${name}-`));
}

describe("WorktreePool", () => {
  let primary: string;
  let poolDir: string;

  beforeEach(() => {
    primary = tmpDir("primary");
    poolDir = path.join(primary, "..", path.basename(primary) + "-pool");
    vi.mocked(execa).mockClear();
    vi.mocked(execa).mockResolvedValue({
      exitCode: 0,
      stdout: "",
      stderr: "",
    } as Awaited<ReturnType<typeof execa>>);
  });

  afterEach(() => {
    fs.rmSync(primary, { recursive: true, force: true });
    fs.rmSync(poolDir, { recursive: true, force: true });
  });

  it("rejects maxConcurrent < 1", () => {
    expect(
      () => new WorktreePool({ primaryWorktreePath: primary, poolDir, maxConcurrent: 0 })
    ).toThrow();
  });

  it("slot 0 hands out the primary worktree path without spawning git", async () => {
    const pool = new WorktreePool({
      primaryWorktreePath: primary,
      poolDir,
      maxConcurrent: 1,
    });
    const lease = await pool.acquire(1234, "feat/foo");
    expect(lease).not.toBeNull();
    expect(lease!.slotId).toBe(0);
    expect(lease!.worktreePath).toBe(primary);
    expect(execa).not.toHaveBeenCalled();
  });

  it("slots 1..N-1 materialise lazily via git worktree add", async () => {
    const pool = new WorktreePool({
      primaryWorktreePath: primary,
      poolDir,
      maxConcurrent: 3,
    });
    const a = await pool.acquire(1, "feat/a");
    const b = await pool.acquire(2, "feat/b");
    const c = await pool.acquire(3, "feat/c");
    expect(a!.slotId).toBe(0);
    expect(b!.slotId).toBe(1);
    expect(c!.slotId).toBe(2);
    expect(b!.worktreePath).toContain("wt-1");
    expect(c!.worktreePath).toContain("wt-2");
    // git worktree add ran exactly twice (slots 1 and 2; slot 0 is primary).
    expect(execa).toHaveBeenCalledTimes(2);
  });

  it("returns null when the pool is full", async () => {
    const pool = new WorktreePool({
      primaryWorktreePath: primary,
      poolDir,
      maxConcurrent: 2,
    });
    await pool.acquire(1, "feat/a");
    await pool.acquire(2, "feat/b");
    const overflow = await pool.acquire(3, "feat/c");
    expect(overflow).toBeNull();
  });

  it("release returns a slot to the free pool", async () => {
    const pool = new WorktreePool({
      primaryWorktreePath: primary,
      poolDir,
      maxConcurrent: 2,
    });
    const a = await pool.acquire(1, "feat/a");
    const b = await pool.acquire(2, "feat/b");
    expect(pool.freeSlotCount()).toBe(0);
    pool.release(a!);
    expect(pool.freeSlotCount()).toBe(1);
    pool.release(b!);
    expect(pool.freeSlotCount()).toBe(2);
  });

  it("refuses to release a pinned lease", async () => {
    const pool = new WorktreePool({
      primaryWorktreePath: primary,
      poolDir,
      maxConcurrent: 1,
    });
    const lease = await pool.acquire(1, "feat/a");
    pool.pin(lease!);
    expect(() => pool.release(lease!)).toThrow();
    pool.unpin(lease!);
    pool.release(lease!);
    expect(pool.freeSlotCount()).toBe(1);
  });

  it("findLeaseByPr returns the live lease", async () => {
    const pool = new WorktreePool({
      primaryWorktreePath: primary,
      poolDir,
      maxConcurrent: 2,
    });
    const lease = await pool.acquire(42, "feat/x");
    expect(pool.findLeaseByPr(42)).toBe(lease);
    expect(pool.findLeaseByPr(99)).toBeNull();
  });

  it("reattachPinned re-leases a slot on supervisor restart", () => {
    const pool = new WorktreePool({
      primaryWorktreePath: primary,
      poolDir,
      maxConcurrent: 3,
    });
    const lease = pool.reattachPinned(7, 2, "feat/x");
    expect(lease).not.toBeNull();
    expect(lease!.slotId).toBe(2);
    expect(lease!.pinned).toBe(true);
    expect(pool.freeSlotCount()).toBe(2);
  });

  it("reattachPinned returns null for an out-of-range slot", () => {
    const pool = new WorktreePool({
      primaryWorktreePath: primary,
      poolDir,
      maxConcurrent: 2,
    });
    // slot 5 doesn't exist when maxConcurrent is 2
    expect(pool.reattachPinned(7, 5, "feat/x")).toBeNull();
  });

  it("two concurrent acquires for an empty pool don't collide on the same slot", async () => {
    const pool = new WorktreePool({
      primaryWorktreePath: primary,
      poolDir,
      maxConcurrent: 2,
    });
    const [a, b] = await Promise.all([
      pool.acquire(1, "feat/a"),
      pool.acquire(2, "feat/b"),
    ]);
    expect(a).not.toBeNull();
    expect(b).not.toBeNull();
    expect(a!.slotId).not.toBe(b!.slotId);
  });
});
