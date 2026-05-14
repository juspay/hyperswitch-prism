import { describe, expect, it } from "vitest";
import { parseConnectorAndFlow, parseTaskListIssueRefs } from "../src/github/tree.js";

describe("parseConnectorAndFlow", () => {
  it("parses bracket convention", () => {
    expect(parseConnectorAndFlow("[parity] stripe / capture / amount.currency")).toEqual({
      connector: "stripe",
      flow: "capture",
    });
  });
  it("parses paren convention", () => {
    expect(parseConnectorAndFlow("parity(adyen/authorize): metadata mismatch")).toEqual({
      connector: "adyen",
      flow: "authorize",
    });
  });
  it("parses colon convention", () => {
    expect(parseConnectorAndFlow("stripe: capture flow returns wrong currency")).toEqual({
      connector: "stripe",
      flow: "capture",
    });
  });
  it("falls back to unknown/unknown on no match", () => {
    expect(parseConnectorAndFlow("random title")).toEqual({ connector: "unknown", flow: "unknown" });
  });
});

describe("parseTaskListIssueRefs", () => {
  it("extracts #N refs", () => {
    const body = "Children:\n- [ ] #100\n- [x] #101\nrandom #102 in prose";
    const refs = parseTaskListIssueRefs(body, "juspay", "hyperswitch-cloud");
    const nums = refs.map((r) => r.number).sort();
    expect(nums).toContain(100);
    expect(nums).toContain(101);
  });
  it("extracts full URLs with different repo", () => {
    const body = "- [ ] https://github.com/foo/bar/issues/55";
    const refs = parseTaskListIssueRefs(body, "juspay", "hyperswitch-cloud");
    expect(refs.find((r) => r.number === 55)?.repo).toBe("foo/bar");
  });
});
