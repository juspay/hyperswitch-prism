import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../src/github/labels.js", async () => {
  const actual = await vi.importActual<typeof import("../src/github/labels.js")>("../src/github/labels.js");
  return {
    ...actual,
    transition: vi.fn(async () => ({ raced: false })),
  };
});

vi.mock("../src/github/client.js", () => ({
  commentIssue: vi.fn(async () => {}),
  viewPr: vi.fn(async () => ({
    number: 9,
    state: "open",
    mergedAt: null,
    url: "https://github.com/x/y/pull/9",
    headRefName: "feat/x",
    author: { login: "10X-GRACE" },
    statusCheckRollup: [],
    createdAt: new Date().toISOString(),
  })),
}));

import { runSweep } from "../src/phases/sweep.js";
import { LABELS, transition } from "../src/github/labels.js";
import type { Leaf } from "../src/types.js";
import type { ParityConfig } from "../src/config.js";

const cfg = {
  github: { owner: "juspay", repo: "hyperswitch-cloud", rootIssue: 15576, actor: "10X-GRACE" },
  heartbeat: { sweepStalePrDays: 7 },
} as unknown as ParityConfig;

function mkLeaf(over: Partial<Leaf> = {}): Leaf {
  return {
    number: 200,
    title: "[parity] stripe / capture",
    body: "",
    labels: [],
    createdAt: "2025-01-01T00:00:00Z",
    url: "https://github.com/juspay/hyperswitch-cloud/issues/200",
    state: "OPEN",
    linkedPRs: [],
    parentTracking: 15576,
    connector: "stripe",
    flow: "capture",
    ...over,
  };
}

describe("runSweep auto-close", () => {
  beforeEach(() => {
    vi.mocked(transition).mockClear();
  });

  it("closes the issue when a linked PR is merged and the issue is still OPEN", async () => {
    const leaf = mkLeaf({
      state: "OPEN",
      labels: [LABELS.PR_OPEN, LABELS.CLAIMED],
      linkedPRs: [
        { repo: "juspay/hyperswitch-prism", number: 1234, state: "merged", url: "https://x/pr/1234", author: "10X-GRACE" },
      ],
    });

    const report = await runSweep(cfg, [leaf]);

    expect(transition).toHaveBeenCalledTimes(1);
    const call = vi.mocked(transition).mock.calls[0][0];
    expect(call.close).toEqual({ reason: "completed" });
    expect(call.add).toContain(LABELS.MERGED);
    expect(call.remove).toEqual(expect.arrayContaining([LABELS.PR_OPEN, LABELS.CLAIMED]));
    expect(call.comment).toMatch(/Auto-closed by parity autopilot/);
    expect(report.autoClosed).toBe(1);
    expect(report.mergedClaimed).toBe(1);
  });

  it("closes regardless of PR author (honors 'any merged PR' policy)", async () => {
    const leaf = mkLeaf({
      state: "OPEN",
      linkedPRs: [
        { repo: "juspay/hyperswitch-prism", number: 1234, state: "merged", url: "https://x/pr/1234", author: "someone-else" },
      ],
    });

    await runSweep(cfg, [leaf]);

    expect(transition).toHaveBeenCalledTimes(1);
    expect(vi.mocked(transition).mock.calls[0][0].close).toEqual({ reason: "completed" });
  });

  it("does NOT close when the issue is already CLOSED on GitHub", async () => {
    const leaf = mkLeaf({
      state: "CLOSED",
      labels: [LABELS.MERGED],
      linkedPRs: [
        { repo: "juspay/hyperswitch-prism", number: 1234, state: "merged", url: "https://x/pr/1234", author: "10X-GRACE" },
      ],
    });

    const report = await runSweep(cfg, [leaf]);

    expect(transition).not.toHaveBeenCalled();
    expect(report.autoClosed).toBe(0);
  });

  it("does NOT close when no merged PR exists (open PR only)", async () => {
    const leaf = mkLeaf({
      state: "OPEN",
      linkedPRs: [
        { repo: "juspay/hyperswitch-prism", number: 1234, state: "open", url: "https://x/pr/1234", author: "10X-GRACE" },
      ],
    });

    const report = await runSweep(cfg, [leaf]);

    // Either no transition at all, or a transition without close.
    for (const call of vi.mocked(transition).mock.calls) {
      expect(call[0].close).toBeUndefined();
    }
    expect(report.autoClosed).toBe(0);
  });

  it("posts close-only transition when MERGED label already present but issue still OPEN", async () => {
    const leaf = mkLeaf({
      state: "OPEN",
      labels: [LABELS.MERGED],
      linkedPRs: [
        { repo: "juspay/hyperswitch-prism", number: 1234, state: "merged", url: "https://x/pr/1234", author: "10X-GRACE" },
      ],
    });

    await runSweep(cfg, [leaf]);

    expect(transition).toHaveBeenCalledTimes(1);
    const call = vi.mocked(transition).mock.calls[0][0];
    // No-op label changes — label already correct, just close + comment.
    expect(call.add).toEqual([]);
    expect(call.remove).toEqual([]);
    expect(call.close).toEqual({ reason: "completed" });
  });
});
