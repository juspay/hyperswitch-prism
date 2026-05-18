import { describe, expect, it } from "vitest";
import { deriveLocusFromComments, deriveStatus, detectLabelBackfills } from "../src/dashboard/derive.js";
import { LABELS } from "../src/github/labels.js";
import type { Leaf } from "../src/types.js";

function mkLeaf(over: Partial<Leaf> = {}): Leaf {
  return {
    number: 1,
    title: "t",
    body: "",
    labels: [],
    createdAt: "2025-01-01T00:00:00Z",
    url: "https://github.com/x/y/issues/1",
    state: "OPEN",
    linkedPRs: [],
    parentTracking: 1000,
    connector: "stripe",
    flow: "capture",
    ...over,
  };
}

describe("deriveStatus", () => {
  it("returns pr-merged when MERGED label present", () => {
    expect(deriveStatus(mkLeaf({ labels: [LABELS.MERGED] }))).toBe("pr-merged");
  });
  it("returns pr-merged when any linked PR merged", () => {
    expect(deriveStatus(mkLeaf({ linkedPRs: [{ repo: "x/y", number: 9, state: "merged" }] }))).toBe("pr-merged");
  });
  it("returns pr-open when open PR linked", () => {
    expect(deriveStatus(mkLeaf({ linkedPRs: [{ repo: "x/y", number: 9, state: "open" }] }))).toBe("pr-open");
  });
  it("returns blocked when BLOCKED label present", () => {
    expect(deriveStatus(mkLeaf({ labels: [LABELS.BLOCKED] }))).toBe("blocked");
  });
  it("returns not-applicable when SKIP label present", () => {
    expect(deriveStatus(mkLeaf({ labels: [LABELS.SKIP] }))).toBe("not-applicable");
  });
  it("returns no-pr by default", () => {
    expect(deriveStatus(mkLeaf())).toBe("no-pr");
  });

  it("returns pr-merged when issue CLOSED with a merged linked PR", () => {
    expect(
      deriveStatus(mkLeaf({ state: "CLOSED", linkedPRs: [{ repo: "x/y", number: 9, state: "merged" }] })),
    ).toBe("pr-merged");
  });

  it("returns closed when issue CLOSED without any merged PR", () => {
    expect(deriveStatus(mkLeaf({ state: "CLOSED" }))).toBe("closed");
  });

  it("returns closed when issue CLOSED with an open-only PR (e.g., dupe close)", () => {
    expect(
      deriveStatus(mkLeaf({ state: "CLOSED", linkedPRs: [{ repo: "x/y", number: 9, state: "open" }] })),
    ).toBe("closed");
  });

  it("CLOSED state overrides labels (e.g., stale parity:fix-pr-open label on a closed issue)", () => {
    expect(
      deriveStatus(mkLeaf({ state: "CLOSED", labels: [LABELS.PR_OPEN], linkedPRs: [] })),
    ).toBe("closed");
  });
});

describe("deriveLocusFromComments", () => {
  it("returns null when no understanding summary exists", () => {
    expect(deriveLocusFromComments([{ body: "hello", createdAt: "2025-01-01" }])).toBeNull();
  });
  it("parses single prism-transformer tick", () => {
    const body = `## Understanding Summary\n\n### Root-cause Locus\n- [x] Prism transformer (foo)\n- [ ] Hyperswitch UCS bridge\n- [ ] Hyperswitch connector\n- [ ] Ambiguous`;
    expect(deriveLocusFromComments([{ body, createdAt: "2025-01-01" }])).toBe("prism-transformer");
  });
  it("returns ambiguous when two boxes ticked", () => {
    const body = `## Understanding Summary\n- [x] Prism transformer\n- [x] Hyperswitch UCS bridge`;
    expect(deriveLocusFromComments([{ body, createdAt: "2025-01-01" }])).toBe("ambiguous");
  });
});

describe("detectLabelBackfills", () => {
  it("fills MERGED when PR merged but label missing", () => {
    const fix = detectLabelBackfills(
      mkLeaf({ labels: [LABELS.PR_OPEN], linkedPRs: [{ repo: "x/y", number: 1, state: "merged" }] }),
    );
    expect(fix?.add).toContain(LABELS.MERGED);
    expect(fix?.remove).toContain(LABELS.PR_OPEN);
  });
  it("fills PR_OPEN when open PR exists but label missing", () => {
    const fix = detectLabelBackfills(mkLeaf({ linkedPRs: [{ repo: "x/y", number: 1, state: "open" }] }));
    expect(fix?.add).toContain(LABELS.PR_OPEN);
  });
  it("returns null when state matches labels", () => {
    expect(
      detectLabelBackfills(
        mkLeaf({ labels: [LABELS.MERGED], linkedPRs: [{ repo: "x/y", number: 1, state: "merged" }] }),
      ),
    ).toBeNull();
  });
});
