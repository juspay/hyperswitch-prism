import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../src/github/labels.js", async () => {
  const actual = await vi.importActual<typeof import("../src/github/labels.js")>("../src/github/labels.js");
  return {
    ...actual,
    transition: vi.fn(async () => ({ raced: false })),
  };
});

import { escalate } from "../src/escalation.js";
import { LABELS, transition } from "../src/github/labels.js";
import type { ParityConfig } from "../src/config.js";

const cfg = {
  github: { owner: "juspay", repo: "hyperswitch-cloud", rootIssue: 15576, actor: "10X-GRACE" },
} as unknown as ParityConfig;

describe("escalate", () => {
  beforeEach(() => {
    vi.mocked(transition).mockClear();
  });

  it("applies parity:blocked label with audit comment in real (non-dry-run) mode", async () => {
    await escalate({
      cfg,
      issue: 15800,
      step: "plan",
      blocker: "Agent reported plan-time gaps",
      tried: "ran PLAN once",
      question: "Fix is in a forbidden path",
    });

    expect(transition).toHaveBeenCalledTimes(1);
    const call = vi.mocked(transition).mock.calls[0][0];
    expect(call.repo).toBe("juspay/hyperswitch-cloud");
    expect(call.issue).toBe(15800);
    expect(call.add).toEqual([LABELS.BLOCKED]);
    expect(call.comment).toMatch(/Autopilot Escalation:/);
    expect(call.comment).toMatch(/Blocked At/);
    expect(call.comment).toMatch(/plan/);
    expect(call.close).toBeUndefined();
  });

  it("does NOT touch GitHub in dry-run mode", async () => {
    const logSpy = vi.spyOn(console, "log").mockImplementation(() => {});
    try {
      await escalate({
        cfg,
        issue: 15800,
        step: "plan",
        blocker: "blocked",
        tried: "tried",
        question: "?",
        dryRun: true,
      });
      expect(transition).not.toHaveBeenCalled();
      expect(logSpy).toHaveBeenCalledWith(
        expect.stringContaining("[dry-run] escalation (would post + apply parity:blocked on #15800)"),
      );
    } finally {
      logSpy.mockRestore();
    }
  });
});
