import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { credNeedles, findLeaks, issueNeedles } from "../packages/core/dist/scrub.js";

const kinds = (text, needles) => findLeaks(text, needles).map((l) => l.kind);

test("prod identifiers seen in public GRACE PRs are caught", { timeout: 1000 }, () => {
  for (const s of [
    "Closes juspay/hyperswitch-cloud#99991",
    "https://github.com/juspay/hyperswitch-cloud/issues/99992",
    "prod merchant `merchant_1000000000000`",
    "x-request-id `01900000-0000-7000-8000-000000000000`",
    "(merchant `acmefoods`, connector `stripe`, 123 occurrences)",
    "merchant=examplemart body.keyDiff.consumerAuthenticationInformation",
    "sample pay_AAAAAAAAAAAAAAAAAAAA",
    "x-api-secret: sk_live_exampleonly",
    "Authorization: Bearer EXAMPLEEXAMPLEEXAMPLEEXAMPLE",
    "api_key snd_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
  ]) {
    assert.ok(findLeaks(s).length > 0, s);
  }
});

test("ordinary parity PR text passes", { timeout: 1000 }, () => {
  const body = [
    "## Summary",
    "Parity fix for stripe/capture: align the UCS response with hyperswitch.",
    "body.keyDiff.merchantDefinedInformation[3].value — merchant defined information ordering.",
    "pub struct Req { merchant: MerchantInfo, merchant_id: String }",
    "let auth = format!(\"Bearer {}\", token);",
  ].join("\n");
  assert.deepEqual(findLeaks(body), []);
});

test("private issue identifiers become needles, prose and connector/flow do not", { timeout: 1000 }, () => {
  const issue =
    "[prod] Validation router diff: bu_xx_demo / nexixpay / psync\n" +
    "merchant `acmefoods` (org: `ExampleOrg`), merchant defined information, demo_ws_00000000-ab";
  const n = issueNeedles(issue, ["nexixpay", "psync"]);
  for (const want of ["bu_xx_demo", "acmefoods", "ExampleOrg", "demo_ws_00000000-ab"]) assert.ok(n.includes(want), want);
  for (const bad of ["defined", "nexixpay", "psync"]) assert.ok(!n.includes(bad), bad);
  assert.deepEqual(kinds("Shadow diff for Acmefoods on capture", n), ["needle"]);
});

test("creds.json values and their base64 are needles", { timeout: 1000 }, () => {
  const dir = mkdtempSync(join(tmpdir(), "scrub-"));
  const p = join(dir, "creds.json");
  writeFileSync(p, JSON.stringify({ nmi: { api_key: "liveKey12345678", auth_type: "HeaderKey", base_url: "https://x.y" } }));
  const n = credNeedles(p);
  assert.ok(n.includes("liveKey12345678"));
  assert.ok(n.includes(Buffer.from("liveKey12345678").toString("base64")));
  assert.ok(!n.includes("HeaderKey") && !n.includes("https://x.y"));
  assert.deepEqual(credNeedles(join(dir, "missing.json")), []);
});

test("patterns stay linear on long input", { timeout: 1000 }, () => {
  findLeaks("merchant" + " ".repeat(30_000) + "`" + "a".repeat(30_000));
  issueNeedles("org" + ":".repeat(30_000) + "diff:" + "x / ".repeat(10_000));
});

test("parity handoff refuses to publish a summary that names the merchant", { timeout: 5000 }, async () => {
  const { runHandoff } = await import("../packages/parity-core/dist/phases/handoff.js");
  const leaf = {
    number: 99993, title: "[prod] Validation router diff: acmefoods / stripe / capture", url: "https://github.com/juspay/hyperswitch-cloud/issues/99993",
    body: "merchant `acmefoods` (org: `ExampleOrg`) header.keyDiff.stripe-account, 12 occurrences", labels: [], createdAt: "", state: "OPEN",
    linkedPRs: [], parentTracking: 0, connector: "stripe", flow: "capture",
  };
  const exec = { ok: true, target: "prism", repoRoot: "/nonexistent", branch: "b", changedFiles: [], buildTail: "", clippyTail: "", testTail: "" };
  const base = { cfg: { credsPath: "" }, leaf, planMarkdown: "## Implementation Plan", exec, verify: { ok: true, markdown: "" }, dryRun: true };

  const leaky = await runHandoff({ ...base, understanding: { ok: true, markdown: "Shadow diff for Acmefoods on stripe/capture" } });
  assert.equal(leaky.ok, false);
  assert.equal(leaky.escalation.blocker, "public-output scrub gate");

  const clean = await runHandoff({ ...base, understanding: { ok: true, markdown: "header.keyDiff.stripe-account missing on capture" } });
  assert.equal(clean.ok, true);
});
