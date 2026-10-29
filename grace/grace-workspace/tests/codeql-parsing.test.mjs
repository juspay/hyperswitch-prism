import assert from "node:assert/strict";
import test from "node:test";
import { stripJsonFences } from "../packages/core/dist/utils.js";
import { parseConnectorAndFlow } from "../packages/parity-core/dist/github/tree.js";
import { extractPayload } from "../packages/parity-core/dist/verify/payloads.js";
import { renderConnectorMarkdown } from "../packages/parity-core/dist/dashboard/renderer.js";

test("JSON fences are extracted without regex backtracking", { timeout: 1000 }, () => {
  assert.equal(stripJsonFences('```json\n{"ok":true}\n```'), '{"ok":true}');
  assert.equal(stripJsonFences('Result: ```json\n{"ok":true}\n``` done'), '{"ok":true}');
  assert.equal(stripJsonFences("```" + " ".repeat(30_000) + "x"), "```" + " ".repeat(30_000) + "x");
});

test("parity titles are parsed without scanning backtracking", { timeout: 1000 }, () => {
  assert.deepEqual(parseConnectorAndFlow("parity(stripe/capture): mismatch"), { connector: "stripe", flow: "capture" });
  assert.deepEqual(parseConnectorAndFlow("[parity] stripe / capture / amount"), { connector: "stripe", flow: "capture" });
  assert.deepEqual(parseConnectorAndFlow("[parity]" + "!".repeat(30_000)), { connector: "unknown", flow: "unknown" });
});

test("payload and expected JSON blocks are parsed", { timeout: 1000 }, async () => {
  const body = '```json\n{"amount":1}\n```\nExpected oracle fields: ```json\n{"status":"ok"}\n```\nRPC: Payments / Authorize\nProto: payments.proto';
  const spec = await extractPayload({}, { body, connector: "stripe", flow: "authorize" });
  assert.deepEqual(spec?.payload, { amount: 1 });
  assert.deepEqual(spec?.expected, { status: "ok" });
});

test("Markdown table titles escape backslashes before pipes", () => {
  const leaf = {
    number: 42, title: "value\\| extra\nrow", body: "", labels: [], createdAt: "2026-01-01T00:00:00Z",
    url: "", state: "OPEN", linkedPRs: [], parentTracking: 1, connector: "stripe", flow: "capture",
  };
  assert.match(renderConnectorMarkdown("stripe", [leaf]), /value\\\\\\\| extra row/);
});
