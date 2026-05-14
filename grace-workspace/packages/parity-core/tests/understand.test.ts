import { describe, expect, it } from "vitest";
import { parseUnderstandingSummary } from "../src/phases/understand.js";
import { parsePlan } from "../src/phases/plan.js";

const GOOD_SUMMARY = `## Understanding Summary

### Issue Digest
- stripe / capture / amount.currency / mapping

### Oracle Behavior (hyperswitch)
- File \`crates/hyperswitch_connectors/src/connectors/stripe.rs:L120-L135\`, function \`build_capture\`
- Sets currency to lowercase ISO.

### Prism Current Behavior
- File \`crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:L88-L99\`
- Sets currency to uppercase ISO.

### UCS Bridge Behavior
- File \`crates/external_services/src/grpc_client/unified_connector_service.rs:L210-L240\`, function \`forward_capture\`
- Passes through verbatim.

### Divergence Analysis
- Prism uppercases, oracle lowercases. Stripe expects lowercase per docs.

### Root-cause Locus
- [x] Prism transformer (crates/integrations/connector-integration/src/connectors/stripe/transformers.rs)
- [ ] Hyperswitch UCS bridge
- [ ] Hyperswitch connector
- [ ] Ambiguous

Edit target repo: prism

### Collateral Impact
- None.

### Understanding Confidence: HIGH
`;

describe("parseUnderstandingSummary", () => {
  it("parses HIGH-confidence prism-transformer summary", () => {
    const r = parseUnderstandingSummary(GOOD_SUMMARY);
    expect(r.ok).toBe(true);
    expect(r.locus).toBe("prism-transformer");
    expect(r.confidence).toBe("HIGH");
    expect(r.declaredTarget).toBe("prism");
  });
  it("escalates when Need Info emitted", () => {
    const r = parseUnderstandingSummary("## Need Info\n\n- can't find oracle\n");
    expect(r.ok).toBe(false);
    expect(r.escalation?.blocker).toMatch(/insufficient info/i);
  });
  it("escalates on no checkbox", () => {
    const text = GOOD_SUMMARY.replace("[x] Prism transformer", "[ ] Prism transformer");
    const r = parseUnderstandingSummary(text);
    expect(r.ok).toBe(false);
  });
  it("escalates on multiple checkboxes", () => {
    const text = GOOD_SUMMARY.replace("[ ] Hyperswitch UCS bridge", "[x] Hyperswitch UCS bridge");
    const r = parseUnderstandingSummary(text);
    expect(r.ok).toBe(false);
    expect(r.escalation?.blocker).toMatch(/multiple/i);
  });
  it("escalates on MEDIUM confidence", () => {
    const text = GOOD_SUMMARY.replace("Confidence: HIGH", "Confidence: MEDIUM");
    const r = parseUnderstandingSummary(text);
    expect(r.ok).toBe(false);
  });
});

describe("parsePlan", () => {
  it("accepts a 100% confidence plan with concrete files", () => {
    const text = `## Implementation Plan

### Target repo
prism

### Files to modify
- \`crates/integrations/connector-integration/src/connectors/stripe/transformers.rs\` — lowercase currency

### Current code
\`\`\`rust
fn x() { y.to_uppercase() }
\`\`\`

### Proposed code
\`\`\`rust
fn x() { y.to_lowercase() }
\`\`\`

### Verification
- cargo build -p connector-integration

### Falsification
- if currency stays uppercase the plan is wrong

### Risk
- none for adyen

### Plan Confidence: 100%
`;
    const r = parsePlan(text);
    expect(r.ok).toBe(true);
    expect(r.files).toContain("crates/integrations/connector-integration/src/connectors/stripe/transformers.rs");
  });
  it("rejects TBD in plan", () => {
    const text = `## Implementation Plan\n\n- TBD\n### Plan Confidence: 100%`;
    const r = parsePlan(text);
    expect(r.ok).toBe(false);
  });
  it("rejects non-100% confidence", () => {
    const text = `## Implementation Plan\n\n### Plan Confidence: 95%`;
    const r = parsePlan(text);
    expect(r.ok).toBe(false);
  });
  it("escalates on Still Missing", () => {
    const r = parsePlan("## Still Missing\n\n- need example payload\n");
    expect(r.ok).toBe(false);
  });
});
