import { execa } from "execa";
import path from "node:path";
import { promises as fs } from "node:fs";
import type { Checkpoint, TestFailure, TestReport } from "../types.js";
import { getConfig } from "../config.js";
import { repairCode } from "../generators/code-repair.js";

export const cypressCheckpoint: Checkpoint = {
  id: "cypress",
  name: "Cypress E2E tests",
  description: "Runs Cypress and parses the JSON report.",
  retryFrom: "compiler",
  timeout: 300_000,
  async run(ctx) {
    const cfg = getConfig().checkpoints.cypress;
    const reportPath = path.join(ctx.task.projectRoot, "cypress-report.json");
    const args = [...cfg.args, "--reporter-options", `output=${reportPath}`];
    if (ctx.options.dryRun) {
      ctx.log(`[cypress] dry-run: would run ${cfg.command} ${args.join(" ")}`, "info");
      return { passed: true };
    }
    try {
      await execa(cfg.command, args, {
        cwd: ctx.task.projectRoot,
        reject: false,
        stdio: "inherit",
      });
    } catch (err) {
      return {
        passed: false,
        errors: [`cypress spawn failed: ${err instanceof Error ? err.message : String(err)}`],
      };
    }
    let report: TestReport;
    try {
      const raw = await fs.readFile(reportPath, "utf-8");
      const parsed = JSON.parse(raw) as {
        stats?: { tests?: number; passes?: number; failures?: number };
        failures?: Array<{ fullTitle?: string; err?: { message?: string } }>;
      };
      const failures: TestFailure[] = (parsed.failures ?? []).map((f) => ({
        testName: f.fullTitle ?? "unknown",
        error: f.err?.message ?? "unknown",
      }));
      report = {
        totalTests: parsed.stats?.tests ?? 0,
        passed: parsed.stats?.passes ?? 0,
        failed: parsed.stats?.failures ?? failures.length,
        failures,
      };
    } catch (err) {
      return {
        passed: false,
        errors: [`Could not parse cypress report: ${err instanceof Error ? err.message : String(err)}`],
      };
    }
    if (report.failed === 0) {
      return { passed: true, artifacts: { cypressReport: report } };
    }
    return {
      passed: false,
      errors: report.failures.map((f) => `${f.testName}: ${f.error}`),
      artifacts: { cypressReport: report },
    };
  },
  async onFail(ctx, result) {
    if (result.errors?.length) {
      try {
        await repairCode(ctx, result.errors);
      } catch {
        /* ignore */
      }
    }
  },
};
