import type { Checkpoint, TestSuiteResult } from "../types.js";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { runClaudeCode } from "../tools/claude-code-runner.js";
import {
  resolveTestSuiteSystem,
  buildTestSuiteUserPayload,
} from "../generators/test-suite-agent-prompt.js";
import { getConfig } from "../config.js";

interface RawTestSuiteResult {
  status?: string;
  reason?: string;
  fixCommit?: string;
  prUrl?: string;
  connector?: string;
}

const VALID_STATUSES: TestSuiteResult["status"][] = [
  "HARDENED",
  "FAILED",
  "SKIPPED",
  "REPORT_TO_MASTER",
  "CREDENTIALS_FIXED",
];

/**
 * test_suite checkpoint — implements grace/workflow/3_test.md.
 *
 * Phase 0 (here): credential check via creds.json in the session worktree.
 *   - No creds → soft-skip with SKIPPED; pipeline continues.
 * Phases 1-4 (delegated to claude-code): run test-prism, analyse, fix
 *   positive-override bugs, push fix branch + open PR, report status.
 *
 * Marked `continueOnFailure: true` because hardening should never block a
 * PR that's already been opened by pr_review.
 */
export const testSuiteCheckpoint: Checkpoint = {
  id: "test_suite",
  name: "Test Suite (Hardening)",
  description:
    "Verify creds + run test-prism + fix positive overrides per grace/workflow/3_test.md",
  retryFrom: "test_suite",
  timeout: 30 * 60 * 1000,
  continueOnFailure: true,
  async run(ctx) {
    const task = ctx.artifacts.task;
    if (!task) return { passed: false, errors: ["Missing task artifact"] };

    const connectorRaw = task.targetConnectors?.[0]?.trim();
    if (!connectorRaw) {
      ctx.log("[test_suite] No connector — skipped.", "info");
      const result: TestSuiteResult = {
        status: "SKIPPED",
        reason: "No connector in task.targetConnectors",
      };
      return { passed: true, output: "SKIPPED — no connector", artifacts: { testSuite: result } };
    }

    const connector = connectorRaw;
    const connectorLc = connector.toLowerCase();
    const projectRoot = task.projectRoot;

    // Phase 0: creds check. The connector key is conventionally lowercase
    // (matches what creds.json stores — e.g. "asiapay", "stripe").
    const credsPath = path.join(projectRoot, "creds.json");
    if (!existsSync(credsPath)) {
      ctx.log(
        `[test_suite] Phase 0: creds.json not found at ${credsPath} — SKIPPED.`,
        "warn",
      );
      const result: TestSuiteResult = {
        status: "SKIPPED",
        reason: `creds.json not found at ${credsPath}`,
        connector: connectorLc,
      };
      return {
        passed: true,
        output: "SKIPPED — no creds.json",
        artifacts: { testSuite: result },
      };
    }

    let creds: Record<string, unknown> = {};
    try {
      const raw = readFileSync(credsPath, "utf-8");
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        creds = parsed as Record<string, unknown>;
      }
    } catch (parseErr) {
      const msg = parseErr instanceof Error ? parseErr.message : String(parseErr);
      ctx.log(
        `[test_suite] Phase 0: creds.json could not be parsed (${msg}) — SKIPPED.`,
        "warn",
      );
      const result: TestSuiteResult = {
        status: "SKIPPED",
        reason: `creds.json malformed: ${msg}`,
        connector: connectorLc,
      };
      return {
        passed: true,
        output: "SKIPPED — creds.json malformed",
        artifacts: { testSuite: result },
      };
    }

    // Match either exact lowercase or case-insensitive — both are seen in practice.
    const hasCreds =
      connectorLc in creds ||
      Object.keys(creds).some((k) => k.toLowerCase() === connectorLc);
    if (!hasCreds) {
      ctx.log(
        `[test_suite] Phase 0: no creds entry for "${connectorLc}" in creds.json — SKIPPED.`,
        "warn",
      );
      const result: TestSuiteResult = {
        status: "SKIPPED",
        reason: `No credentials in creds.json for connector "${connectorLc}"`,
        connector: connectorLc,
      };
      return {
        passed: true,
        output: "SKIPPED — no creds for connector",
        artifacts: { testSuite: result },
      };
    }

    ctx.log(
      `[test_suite] Phase 0: ✓ creds found for ${connectorLc} — proceeding to test run`,
      "success",
    );

    // Phases 1-4: delegate to claude-code with the 3_test.md prompt.
    const cfg = getConfig();
    const branch = (ctx.artifacts.branch as string | undefined) ?? undefined;

    try {
      const { result: raw } = await runClaudeCode<RawTestSuiteResult>({
        skillBody: resolveTestSuiteSystem(projectRoot),
        userPayload: buildTestSuiteUserPayload(connectorLc, projectRoot, {
          testMode: "grpc",
          branch,
          timeoutSeconds: 600,
        }),
        cwd: projectRoot,
        label: "test_suite",
        sessionLabel: `${connectorLc}-test-suite-${Date.now().toString(36)}`,
        timeoutMs: 30 * 60 * 1000,
        model: cfg.claudeCode?.model ?? cfg.llm?.model,
        allowWrite: true,
      });

      const status: TestSuiteResult["status"] = VALID_STATUSES.includes(
        raw.status as TestSuiteResult["status"],
      )
        ? (raw.status as TestSuiteResult["status"])
        : "FAILED";

      const result: TestSuiteResult = {
        status,
        reason: typeof raw.reason === "string" ? raw.reason : undefined,
        fixCommit: typeof raw.fixCommit === "string" ? raw.fixCommit : undefined,
        prUrl: typeof raw.prUrl === "string" ? raw.prUrl : undefined,
        connector: connectorLc,
      };

      ctx.log(
        `[test_suite] Result: ${result.status}${result.reason ? " — " + result.reason : ""}`,
        result.status === "HARDENED" ? "success" : "info",
      );

      return {
        passed: status === "HARDENED" || status === "SKIPPED",
        output: result.reason ?? result.status,
        artifacts: { testSuite: result },
      };
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      ctx.log(`[test_suite] Failed to run: ${msg}`, "error");
      const result: TestSuiteResult = {
        status: "FAILED",
        reason: `Agent invocation error: ${msg}`,
        connector: connectorLc,
      };
      return {
        passed: false,
        output: `FAILED — ${msg}`,
        errors: [msg],
        artifacts: { testSuite: result },
      };
    }
  },
};
