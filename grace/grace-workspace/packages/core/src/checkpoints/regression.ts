import { execa } from "execa";
import type { Checkpoint } from "../types.js";
import { getConfig } from "../config.js";

export const regressionCheckpoint: Checkpoint = {
  id: "regression",
  name: "Regression testing",
  description: "Runs the repo's regression/test suite to confirm nothing pre-existing broke.",
  retryFrom: "cypress",
  timeout: 600_000,
  async run(ctx) {
    const cfg = getConfig().checkpoints.regression;
    if (cfg.enabled === false) {
      ctx.log("[regression] disabled in config.yml — skipping.", "info");
      return { passed: true };
    }
    if (ctx.options.dryRun) {
      ctx.log(`[regression] dry-run: would run ${cfg.command} ${cfg.args.join(" ")}`, "info");
      return { passed: true };
    }
    try {
      const res = await execa(cfg.command, cfg.args, {
        cwd: ctx.task.projectRoot,
        reject: false,
        all: true,
      });
      if (res.exitCode === 0) return { passed: true };
      const out = (res.all ?? res.stderr ?? res.stdout ?? "").toString();
      return {
        passed: false,
        errors: [out.slice(0, 2000) || `regression exited with ${res.exitCode}`],
      };
    } catch (err) {
      return {
        passed: false,
        errors: [`regression spawn failed: ${err instanceof Error ? err.message : String(err)}`],
      };
    }
  },
};
