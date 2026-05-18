import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// We can't easily import loadParityConfig without it also importing @10xgrace/core's loader.
// Sanity check: env-var override path works without touching disk.
describe("env overrides", () => {
  const tmp = mkdtempSync(join(tmpdir(), "parity-config-test-"));
  const cfgPath = join(tmp, "config.yml");

  beforeEach(() => {
    writeFileSync(
      cfgPath,
      `projectRoot: /tmp
devServerUrl: http://x
designMatchThreshold: 0.9
maxRetries: 1
dashboardPort: 1
wsPort: 2
llm:
  baseUrl: ""
  apiKey: ""
  model: ""
  protocol: openai
  maxTokens: 1
  temperature: 0
  timeoutMs: 1
parityAutopilot:
  prismPath: "/no/such/path"
  github:
    owner: juspay
    repo: hyperswitch-cloud
    rootIssue: 15576
    actor: "test"
`,
    );
  });

  afterEach(() => {
    delete process.env.PARITY_PRISM_PATH;
    delete process.env.PARITY_BRIDGE_PATH;
    delete process.env.PARITY_ORACLE_PATH;
  });

  it("rejects nonexistent prismPath", async () => {
    process.env.PARITY_PRISM_PATH = "/no/such/path";
    const mod = await import("../src/config.js");
    expect(() => mod.loadParityConfig(cfgPath)).toThrow(/does not exist/);
  });

  it("accepts env-overridden prismPath", async () => {
    process.env.PARITY_PRISM_PATH = tmp;
    const mod = await import("../src/config.js");
    const cfg = mod.loadParityConfig(cfgPath);
    expect(cfg.prismPath).toBe(tmp);
    expect(cfg.github.rootIssue).toBe(15576);
  });

  it("rejects bridgeWritePath pointing at a non-existent directory", async () => {
    process.env.PARITY_PRISM_PATH = tmp;
    process.env.PARITY_BRIDGE_PATH = "/no/such/bridge";
    const mod = await import("../src/config.js");
    expect(() => mod.loadParityConfig(cfgPath)).toThrow(/bridgeWritePath does not exist/);
  });

  it("rejects bridgeWritePath that is not a git repo with crates/external_services", async () => {
    // Build a directory that exists but is missing .git AND crates/external_services.
    const fakeBridge = mkdtempSync(join(tmpdir(), "fake-bridge-"));
    process.env.PARITY_PRISM_PATH = tmp;
    process.env.PARITY_BRIDGE_PATH = fakeBridge;
    const mod = await import("../src/config.js");
    expect(() => mod.loadParityConfig(cfgPath)).toThrow(/not a git repo/);

    // Now add .git but still no crates/external_services — should fail differently.
    mkdirSync(join(fakeBridge, ".git"));
    expect(() => mod.loadParityConfig(cfgPath)).toThrow(/does not look like a hyperswitch clone/);
  });

  it("rejects oracleReadOnlyPath pointing at a non-existent directory", async () => {
    process.env.PARITY_PRISM_PATH = tmp;
    process.env.PARITY_ORACLE_PATH = "/no/such/oracle";
    const mod = await import("../src/config.js");
    expect(() => mod.loadParityConfig(cfgPath)).toThrow(/oracleReadOnlyPath does not exist/);
  });
});
