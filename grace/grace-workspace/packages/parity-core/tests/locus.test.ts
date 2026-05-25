import { describe, expect, it } from "vitest";
import { branchName, classifyTarget, slugifyTitle, validateDiff } from "../src/locus.js";

const RULES = {
  forbiddenOracleDirs: [
    "crates/hyperswitch_connectors/",
    "crates/hyperswitch_domain_models/",
    "crates/api_models/",
    "crates/router/",
  ],
  forbiddenPrismDirs: ["crates/types-traits/"],
};

describe("classifyTarget", () => {
  it("prism transformer locus → prism", () => {
    expect(classifyTarget({ understoodLocus: "prism-transformer", bridgeAvailable: false }).target).toBe("prism");
  });
  it("hs-bridge locus without bridgeWritePath → escalate", () => {
    const r = classifyTarget({ understoodLocus: "hs-bridge", bridgeAvailable: false });
    expect(r.target).toBe("escalate");
    expect(r.reason).toMatch(/bridgeWritePath/);
  });
  it("hs-bridge with bridge available → hyperswitch-bridge", () => {
    expect(classifyTarget({ understoodLocus: "hs-bridge", bridgeAvailable: true }).target).toBe("hyperswitch-bridge");
  });
  it("hs-connector → escalate (oracle disagreement)", () => {
    expect(classifyTarget({ understoodLocus: "hs-connector", bridgeAvailable: true }).target).toBe("escalate");
  });
  it("ambiguous → escalate", () => {
    expect(classifyTarget({ understoodLocus: "ambiguous", bridgeAvailable: true }).target).toBe("escalate");
  });
  it("declared mismatch flagged as reclassified", () => {
    const r = classifyTarget({ declaredTarget: "prism", understoodLocus: "hs-bridge", bridgeAvailable: true });
    expect(r.target).toBe("hyperswitch-bridge");
    expect(r.reclassified).toBe(true);
  });
});

describe("validateDiff (prism target)", () => {
  it("accepts connector-integration paths", () => {
    expect(
      validateDiff({
        target: "prism",
        rules: RULES,
        changedFiles: ["crates/integrations/connector-integration/src/connectors/stripe/transformers.rs"],
      }).ok,
    ).toBe(true);
  });
  it("rejects types-traits edits", () => {
    const r = validateDiff({
      target: "prism",
      rules: RULES,
      changedFiles: ["crates/types-traits/grpc-api-types/proto/foo.proto"],
    });
    expect(r.ok).toBe(false);
    expect(r.violations.join("\n")).toMatch(/types-traits/);
  });
  it("rejects unrelated path", () => {
    const r = validateDiff({ target: "prism", rules: RULES, changedFiles: ["crates/grpc-server/foo.rs"] });
    expect(r.ok).toBe(false);
  });
});

describe("validateDiff (hyperswitch-bridge target)", () => {
  it("accepts the bridge file", () => {
    expect(
      validateDiff({
        target: "hyperswitch-bridge",
        rules: RULES,
        changedFiles: ["crates/external_services/src/grpc_client/unified_connector_service.rs"],
      }).ok,
    ).toBe(true);
  });
  it("rejects hyperswitch_connectors edit", () => {
    const r = validateDiff({
      target: "hyperswitch-bridge",
      rules: RULES,
      changedFiles: ["crates/hyperswitch_connectors/src/connectors/stripe.rs"],
    });
    expect(r.ok).toBe(false);
    expect(r.violations.join("\n")).toMatch(/hyperswitch_connectors/);
  });
  it("rejects router edit", () => {
    const r = validateDiff({
      target: "hyperswitch-bridge",
      rules: RULES,
      changedFiles: ["crates/router/src/foo.rs"],
    });
    expect(r.ok).toBe(false);
  });
});

describe("branchName + slug", () => {
  it("slugifies title", () => {
    expect(slugifyTitle("[parity] stripe / capture / amount.currency mismatch")).toMatch(/parity-stripe-capture/);
  });
  it("prism branch format", () => {
    expect(branchName("prism", "stripe", "capture", "currency bug")).toBe("parity/stripe/capture-currency-bug");
  });
  it("bridge branch format", () => {
    expect(branchName("hyperswitch-bridge", "adyen", "authorize", "metadata mismatch")).toBe(
      "parity/bridge/adyen-authorize-metadata-mismatch",
    );
  });
});
