import { execa } from "execa";
import type { Checkpoint, TestFailure, TestReport } from "../types.js";
import { getConfig } from "../config.js";
import { repairCode } from "../generators/code-repair.js";

interface PwSuite {
  suites?: PwSuite[];
  specs?: Array<{
    title: string;
    tests: Array<{ results: Array<{ status: string; error?: { message?: string } }> }>;
  }>;
}

function collect(suite: PwSuite, failures: TestFailure[], totals: { total: number; passed: number; failed: number }) {
  for (const spec of suite.specs ?? []) {
    for (const t of spec.tests) {
      for (const r of t.results) {
        totals.total++;
        if (r.status === "passed") totals.passed++;
        else {
          totals.failed++;
          failures.push({ testName: spec.title, error: r.error?.message ?? r.status });
        }
      }
    }
  }
  for (const s of suite.suites ?? []) collect(s, failures, totals);
}

export const playwrightCheckpoint: Checkpoint = {
  id: "playwright",
  name: "Playwright cross-browser tests",
  description: "Runs Playwright and parses the JSON report.",
  retryFrom: "compiler",
  timeout: 300_000,
  async run(ctx) {
    const cfg = getConfig().checkpoints.playwright;
    if (ctx.options.dryRun) {
      ctx.log(`[playwright] dry-run: would run ${cfg.command} ${cfg.args.join(" ")}`, "info");
      return { passed: true };
    }
    let res;
    try {
      res = await execa(cfg.command, cfg.args, {
        cwd: ctx.task.projectRoot,
        reject: false,
        all: true,
      });
    } catch (err) {
      return {
        passed: false,
        errors: [`playwright spawn failed: ${err instanceof Error ? err.message : String(err)}`],
      };
    }
    const stdout = res.stdout ?? "";
    let parsed: { suites?: PwSuite[] };
    try {
      parsed = JSON.parse(stdout);
    } catch {
      return {
        passed: res.exitCode === 0,
        errors: res.exitCode === 0 ? [] : [stdout.slice(0, 1500)],
      };
    }
    const failures: TestFailure[] = [];
    const totals = { total: 0, passed: 0, failed: 0 };
    for (const s of parsed.suites ?? []) collect(s, failures, totals);
    const report: TestReport = {
      totalTests: totals.total,
      passed: totals.passed,
      failed: totals.failed,
      failures,
    };
    if (report.failed === 0) {
      return { passed: true, artifacts: { playwrightReport: report } };
    }
    return {
      passed: false,
      errors: report.failures.map((f) => `${f.testName}: ${f.error}`),
      artifacts: { playwrightReport: report },
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
