import { describe, expect, it } from "vitest";
import { diffField, extractPayload, getByPath } from "../src/verify/payloads.js";
import type { ParityConfig } from "../src/config.js";

const cfg = {
  prismPath: "/tmp/nope",
  oracleReadOnlyPath: "",
  bridgeWritePath: "",
  credsPath: "",
  github: { owner: "x", repo: "y", rootIssue: 1, actor: "" },
  grpc: { port: 8000, metricsPort: 8080, bootTimeoutMs: 30000, callTimeoutMs: 60000 },
  cache: { dir: ".cache", treeTtlMs: 1 },
  llm: { runner: "claude-code", understandModel: "", planModel: "", executeModel: "" },
  heartbeat: { pickOldestFirst: true, maxInflightClaimed: 1, sweepStalePrDays: 7, rcaStaleHours: 24 },
  rules: { forbiddenOracleDirs: [], forbiddenPrismDirs: [] },
} as unknown as ParityConfig;

describe("getByPath / diffField", () => {
  it("walks nested objects", () => {
    expect(getByPath({ a: { b: { c: 7 } } }, "a.b.c")).toBe(7);
  });
  it("walks arrays via [n]", () => {
    expect(getByPath({ xs: [{ y: "z" }] }, "xs.[0].y")).toBe("z");
  });
  it("diffs to match", () => {
    expect(diffField({ a: { b: "lower" } }, "lower", "a.b").match).toBe(true);
  });
  it("diffs to mismatch", () => {
    expect(diffField({ a: { b: "UPPER" } }, "lower", "a.b").match).toBe(false);
  });
});

describe("extractPayload from body", () => {
  it("parses body with JSON, RPC, Proto, Expected", async () => {
    const body = `
Some description.

\`\`\`json
{ "amount": 100, "currency": "usd" }
\`\`\`

RPC: ucs.PaymentService/Authorize
Proto: payment.proto

Expected oracle field:
\`\`\`json
{ "response.currency": "usd" }
\`\`\`
`;
    const spec = await extractPayload(cfg, { body, connector: "stripe", flow: "authorize" });
    expect(spec).not.toBeNull();
    expect(spec!.service).toBe("ucs.PaymentService");
    expect(spec!.rpc).toBe("Authorize");
    expect(spec!.protoFile).toBe("payment.proto");
    expect(spec!.expected["response.currency"]).toBe("usd");
  });
  it("returns null when body has no JSON or RPC", async () => {
    const spec = await extractPayload(cfg, { body: "nothing here", connector: "stripe", flow: "auth" });
    expect(spec).toBeNull();
  });
});
