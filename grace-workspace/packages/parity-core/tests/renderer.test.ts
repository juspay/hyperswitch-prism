import { describe, expect, it } from "vitest";
import { renderConnectorMarkdown, renderDashboardMarkdown } from "../src/dashboard/renderer.js";
import { LABELS } from "../src/github/labels.js";
import type { Leaf } from "../src/types.js";
import type { ParityConfig } from "../src/config.js";

const cfg = {
  github: { owner: "juspay", repo: "hyperswitch-cloud", rootIssue: 15576, actor: "x" },
} as unknown as ParityConfig;

const leaves: Leaf[] = [
  {
    number: 100, title: "[parity] stripe / capture / currency", body: "", labels: [],
    createdAt: "2025-01-02T00:00:00Z", url: "https://x", state: "OPEN", linkedPRs: [],
    parentTracking: 15601, connector: "stripe", flow: "capture",
  },
  {
    number: 101, title: "stripe: authorize amount", body: "", labels: [LABELS.PR_OPEN],
    createdAt: "2025-01-01T00:00:00Z", url: "https://x", state: "OPEN",
    linkedPRs: [{ repo: "x/y", number: 9, state: "open" }],
    parentTracking: 15601, connector: "stripe", flow: "authorize",
  },
];

describe("renderDashboardMarkdown", () => {
  it("includes per-connector counts row", () => {
    const md = renderDashboardMarkdown(leaves, cfg);
    expect(md).toMatch(/# Parity Dashboard/);
    expect(md).toMatch(/stripe/);
    // 1 no-pr, 1 pr-open
    expect(md).toMatch(/\| stripe \| #15601 \| 2 \| 1 \| 1 \|/);
  });

  it("lists backlog in FIFO order by createdAt", () => {
    const md = renderDashboardMarkdown(leaves, cfg);
    const backlogIdx = md.indexOf("Backlog priority");
    expect(backlogIdx).toBeGreaterThan(0);
    // #100 (2025-01-02) is the only no-pr leaf — should appear in backlog
    expect(md.slice(backlogIdx)).toMatch(/stripe #100/);
  });
});

describe("renderConnectorMarkdown", () => {
  it("emits a row per leaf", () => {
    const md = renderConnectorMarkdown("stripe", leaves);
    expect(md).toMatch(/# stripe — tracking #15601/);
    expect(md).toMatch(/\| #100 \| capture \|/);
    expect(md).toMatch(/\| #101 \| authorize \|/);
  });
});
