import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Mock every IO module the orchestrator touches BEFORE importing it.

vi.mock("@10xgrace/core", () => {
  class StateManager {
    db = {
      exec: vi.fn(),
      prepare: vi.fn(() => ({ run: vi.fn(), get: vi.fn(), all: vi.fn(() => []) })),
    };
  }
  return {
    StateManager,
    loadConfig: vi.fn(),
    runAI: vi.fn(),
    // Mirror real signature: runs the inner fn unchanged so phase mocks (below) still resolve.
    withRunner: vi.fn(async (_runner: string, fn: () => Promise<unknown>) => fn()),
  };
});

vi.mock("../src/github/tree.js", () => ({
  walkTree: vi.fn(),
}));

vi.mock("../src/dashboard/renderer.js", () => ({
  writeDashboardFiles: vi.fn(async () => {}),
}));

vi.mock("../src/persistence.js", () => ({
  extendForParity: vi.fn(),
  newHeartbeatId: vi.fn(() => "hb-test"),
  recordHeartbeat: vi.fn(),
  upsertLeaf: vi.fn(),
  saveSessionIds: vi.fn(),
  getLeafRow: vi.fn(() => null),
}));

vi.mock("../src/github/client.js", () => ({
  getIssue: vi.fn(async () => ({ comments: [] })),
  ghJson: vi.fn(),
  editIssueLabels: vi.fn(),
  commentIssue: vi.fn(),
  viewPr: vi.fn(),
  createPrDraft: vi.fn(),
}));

vi.mock("../src/github/labels.js", async () => {
  const actual = await vi.importActual<any>("../src/github/labels.js");
  return {
    ...actual,
    transition: vi.fn(async () => ({ raced: false })),
  };
});

vi.mock("../src/phases/sweep.js", () => ({
  runSweep: vi.fn(async () => ({ fixedLabels: 0, remindersSent: 0, mergedClaimed: 0, blocked: 0 })),
  ownedOpenPRs: vi.fn(() => []),
}));

vi.mock("../src/phases/understand.js", () => ({
  runUnderstand: vi.fn(async () => ({
    ok: true,
    markdown: "## Understanding Summary\n- [x] Prism transformer\n\n### Understanding Confidence: HIGH",
    locus: "prism-transformer",
    confidence: "HIGH",
  })),
}));

vi.mock("../src/phases/plan.js", () => ({
  runPlan: vi.fn(async () => ({
    ok: true,
    markdown: "## Implementation Plan\n\n### Plan Confidence: 100%",
    files: [],
  })),
}));

vi.mock("../src/phases/execute.js", () => ({
  runExecute: vi.fn(async () => ({
    ok: true,
    target: "prism",
    repoRoot: "/tmp/prism",
    branch: "parity/stripe/capture-x",
    changedFiles: ["crates/integrations/connector-integration/src/connectors/stripe/transformers.rs"],
    buildTail: "ok",
    clippyTail: "ok",
    testTail: "ok",
  })),
}));

vi.mock("../src/verify/grpc.js", () => ({
  verifyLeaf: vi.fn(async () => ({ ok: true, markdown: "## gRPC Verification\nPASS" })),
}));

vi.mock("../src/phases/handoff.js", () => ({
  runHandoff: vi.fn(async () => ({ ok: true, prUrl: "https://github.com/juspay/hyperswitch-prism/pull/9999" })),
}));

vi.mock("../src/escalation.js", () => ({
  escalate: vi.fn(async () => {}),
}));

vi.mock("../src/config.js", () => ({
  loadParityConfig: vi.fn(() => ({
    prismPath: "/tmp/prism",
    oracleReadOnlyPath: "",
    bridgeWritePath: "",
    credsPath: "",
    github: { owner: "juspay", repo: "hyperswitch-cloud", rootIssue: 15576, actor: "tester" },
    grpc: { port: 8000, metricsPort: 8080, bootTimeoutMs: 30000, callTimeoutMs: 60000 },
    cache: { dir: ".cache", treeTtlMs: 0 },
    llm: { runner: "claude-code", understandModel: "", planModel: "", executeModel: "" },
    heartbeat: { pickOldestFirst: true, maxInflightClaimed: 1, sweepStalePrDays: 7, rcaStaleHours: 24 },
    rules: { forbiddenOracleDirs: [], forbiddenPrismDirs: [] },
  })),
}));

import { runHeartbeat } from "../src/orchestrator.js";
import { walkTree } from "../src/github/tree.js";
import { transition } from "../src/github/labels.js";
import { runHandoff } from "../src/phases/handoff.js";
import { escalate } from "../src/escalation.js";
import type { Leaf } from "../src/types.js";

// The vi.mock above replaces @10xgrace/core; we pull the mocked withRunner
// out of the module to assert call args.
import * as core from "@10xgrace/core";

function mkLeaf(over: Partial<Leaf> = {}): Leaf {
  return {
    number: 100,
    title: "[parity] stripe / capture / currency",
    body: "",
    labels: [],
    createdAt: "2025-01-02T00:00:00Z",
    url: "https://github.com/juspay/hyperswitch-cloud/issues/100",
    state: "OPEN",
    linkedPRs: [],
    parentTracking: 15601,
    connector: "stripe",
    flow: "capture",
    ...over,
  };
}

describe("runHeartbeat", () => {
  let stdoutWrites: string[] = [];
  let origWrite: typeof process.stdout.write;

  beforeEach(() => {
    vi.clearAllMocks();
    stdoutWrites = [];
    origWrite = process.stdout.write.bind(process.stdout);
    process.stdout.write = ((chunk: any) => {
      stdoutWrites.push(String(chunk));
      return true;
    }) as any;
  });

  afterEach(() => {
    process.stdout.write = origWrite;
  });

  function progressEvents(): any[] {
    const lines = stdoutWrites.join("").split("\n");
    return lines
      .filter((l) => l.startsWith("__PARITY_PROGRESS__ "))
      .map((l) => JSON.parse(l.slice("__PARITY_PROGRESS__ ".length)));
  }

  it("leafNumber override picks the named leaf instead of decideNextLeaf", async () => {
    const a = mkLeaf({ number: 100, createdAt: "2025-01-01T00:00:00Z" });
    const b = mkLeaf({ number: 200, createdAt: "2025-01-02T00:00:00Z" });
    vi.mocked(walkTree).mockResolvedValue([a, b]);

    const r = await runHeartbeat({ leafNumber: 200, dryRun: true });

    expect(r.outcome).toBe("pr-opened");
    expect(r.pickedLeaf).toBe(200);
    const decide = progressEvents().find((e) => e.phase === "decide");
    expect(decide.picked).toBe(200);
  });

  it("returns outcome=error when leafNumber not in tree", async () => {
    vi.mocked(walkTree).mockResolvedValue([mkLeaf({ number: 100 })]);

    const r = await runHeartbeat({ leafNumber: 999999, dryRun: true });

    expect(r.outcome).toBe("error");
    expect(r.detail).toMatch(/not in tree/);
  });

  it("dryRun=true skips CLAIM and final HANDOFF transition (no parity:autopilot-claimed posted)", async () => {
    vi.mocked(walkTree).mockResolvedValue([mkLeaf({ number: 100 })]);

    await runHeartbeat({ leafNumber: 100, dryRun: true });

    // transition() must NEVER have been called with parity:autopilot-claimed in dry-run
    const calls = vi.mocked(transition).mock.calls;
    const claimCalls = calls.filter((c) => c[0].add?.includes("parity:autopilot-claimed"));
    expect(claimCalls).toHaveLength(0);
    const prOpenCalls = calls.filter((c) => c[0].add?.includes("parity:fix-pr-open"));
    expect(prOpenCalls).toHaveLength(0);
    const rcaCalls = calls.filter((c) => c[0].add?.includes("parity:rca-complete"));
    expect(rcaCalls).toHaveLength(0);

    // runHandoff was called with dryRun=true
    const handoffCalls = vi.mocked(runHandoff).mock.calls;
    expect(handoffCalls.at(-1)?.[0].dryRun).toBe(true);
  });

  it("dryRun=true: escalate is called with dryRun flag when a phase fails", async () => {
    vi.mocked(walkTree).mockResolvedValue([mkLeaf({ number: 100 })]);
    const { runUnderstand } = await import("../src/phases/understand.js");
    vi.mocked(runUnderstand).mockResolvedValueOnce({
      ok: false,
      escalation: { blocker: "missing info", tried: "one pass", question: "?" },
    });

    const r = await runHeartbeat({ leafNumber: 100, dryRun: true });

    expect(r.outcome).toBe("escalated");
    expect(vi.mocked(escalate)).toHaveBeenCalledWith(
      expect.objectContaining({ step: "understand", dryRun: true }),
    );
  });

  it("emits progress events in expected order for a successful dry-run", async () => {
    vi.mocked(walkTree).mockResolvedValue([mkLeaf({ number: 100 })]);

    await runHeartbeat({ leafNumber: 100, dryRun: true });

    const phases = progressEvents().map((e) => `${e.phase}:${e.status ?? e.outcome}`);
    // discover → refresh → decide → claim(skipped) → understand → plan → execute → verify → handoff(skipped) → done
    expect(phases[0]).toBe("discover:start");
    expect(phases).toContain("discover:ok");
    expect(phases).toContain("refresh:ok");
    expect(phases).toContain("decide:ok");
    expect(phases).toContain("claim:skipped-dry-run");
    expect(phases).toContain("understand:start");
    expect(phases).toContain("understand:ok");
    expect(phases).toContain("plan:start");
    expect(phases).toContain("plan:ok");
    expect(phases).toContain("execute:start");
    expect(phases).toContain("execute:ok");
    expect(phases).toContain("verify:start");
    expect(phases).toContain("verify:ok");
    expect(phases).toContain("handoff:skipped-dry-run");
    expect(phases).toContain("done:pr-opened");
  });

  it("dryRun=false: claim transition IS posted and runHandoff receives dryRun=false", async () => {
    vi.mocked(walkTree).mockResolvedValue([mkLeaf({ number: 100, labels: [] })]);

    await runHeartbeat({ leafNumber: 100, dryRun: false });

    const calls = vi.mocked(transition).mock.calls;
    const claimCalls = calls.filter((c) => c[0].add?.includes("parity:autopilot-claimed"));
    expect(claimCalls.length).toBeGreaterThan(0);

    const handoffCall = vi.mocked(runHandoff).mock.calls.at(-1);
    expect(handoffCall?.[0].dryRun).toBeFalsy();
  });

  it("runnerOverride threads through to phases via effective cfg", async () => {
    // Phases are mocked so they don't actually call withRunner; instead we assert
    // the orchestrator built an effective cfg with the overridden runner and passed
    // it down. We snoop the cfg that runUnderstand receives.
    vi.mocked(walkTree).mockResolvedValue([mkLeaf({ number: 100 })]);
    const { runUnderstand } = await import("../src/phases/understand.js");

    await runHeartbeat({ leafNumber: 100, dryRun: true, runnerOverride: "opencode" });

    const understandCall = vi.mocked(runUnderstand).mock.calls.at(-1);
    expect(understandCall?.[0].cfg.llm.runner).toBe("opencode");
  });

  it("no runnerOverride: phases see cfg.llm.runner unchanged from loadParityConfig (claude-code default)", async () => {
    vi.mocked(walkTree).mockResolvedValue([mkLeaf({ number: 100 })]);
    const { runUnderstand } = await import("../src/phases/understand.js");

    await runHeartbeat({ leafNumber: 100, dryRun: true });

    const understandCall = vi.mocked(runUnderstand).mock.calls.at(-1);
    expect(understandCall?.[0].cfg.llm.runner).toBe("claude-code");
  });

  it("bridgePathOverride threads into cfg passed to runExecute", async () => {
    vi.mocked(walkTree).mockResolvedValue([mkLeaf({ number: 100 })]);
    const { runExecute } = await import("../src/phases/execute.js");

    await runHeartbeat({
      leafNumber: 100,
      dryRun: true,
      bridgePathOverride: "/from-cli/hyperswitch",
    });

    const execCall = vi.mocked(runExecute).mock.calls.at(-1);
    expect(execCall?.[0].cfg.bridgeWritePath).toBe("/from-cli/hyperswitch");
  });

  it("oraclePathOverride threads into cfg passed to runExecute", async () => {
    vi.mocked(walkTree).mockResolvedValue([mkLeaf({ number: 100 })]);
    const { runExecute } = await import("../src/phases/execute.js");

    await runHeartbeat({
      leafNumber: 100,
      dryRun: true,
      oraclePathOverride: "/from-cli/oracle",
    });

    const execCall = vi.mocked(runExecute).mock.calls.at(-1);
    expect(execCall?.[0].cfg.oracleReadOnlyPath).toBe("/from-cli/oracle");
  });

  it("verifyLeaf is called with exec.target so hs-bridge can short-circuit", async () => {
    vi.mocked(walkTree).mockResolvedValue([mkLeaf({ number: 100 })]);
    const { runExecute } = await import("../src/phases/execute.js");
    const { verifyLeaf } = await import("../src/verify/grpc.js");

    // Override execute to return target=hyperswitch-bridge for this test
    vi.mocked(runExecute).mockResolvedValueOnce({
      ok: true,
      target: "hyperswitch-bridge",
      repoRoot: "/tmp/hyperswitch",
      branch: "parity/bridge/test",
      changedFiles: ["crates/external_services/src/grpc_client/unified_connector_service.rs"],
      buildTail: "ok",
      clippyTail: "ok",
      testTail: "ok",
    });

    await runHeartbeat({ leafNumber: 100, dryRun: true });

    const verifyCall = vi.mocked(verifyLeaf).mock.calls.at(-1);
    expect(verifyCall?.[2]).toBe("hyperswitch-bridge");
  });
});

// Sanity: the @10xgrace/core mock exported a `withRunner` so phase modules that
// import it (when run un-mocked) wouldn't blow up at module-load time.
import { describe as _d, it as _i, expect as _e } from "vitest";
_d("mock surface", () => {
  _i("exposes withRunner", () => {
    _e(typeof (core as any).withRunner).toBe("function");
  });
});
