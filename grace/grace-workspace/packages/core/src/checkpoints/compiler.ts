import { execa } from "execa";
import type { Checkpoint } from "../types.js";
import { getConfig } from "../config.js";
import { repairCode } from "../generators/code-repair.js";

export const compilerCheckpoint: Checkpoint = {
  id: "compiler",
  name: "Compiler check",
  description: "Runs the ReScript build (and optionally ESLint) in the target project.",
  retryFrom: "implementation",
  timeout: 300_000,
  async run(ctx) {
    const { compiler } = getConfig().checkpoints;
    if (ctx.options.dryRun) {
      ctx.log(`[compiler] dry-run: would run ${compiler.command} ${compiler.args.join(" ")}`, "info");
      return { passed: true, artifacts: { compiledFiles: [] } };
    }
    ctx.log(
      `[compiler] ${compiler.command} ${compiler.args.join(" ")} (cwd=${ctx.task.projectRoot})`,
      "info"
    );
    try {
      const res = await execa(compiler.command, compiler.args, {
        cwd: ctx.task.projectRoot,
        reject: false,
        all: true,
      });
      if (res.exitCode === 0) {
        return { passed: true, artifacts: { compiledFiles: [] } };
      }
      const output = (res.all ?? res.stderr ?? res.stdout ?? "").toString();
      const errors = output
        .split("\n")
        .filter((l) => /error|Error|ERR|ReScript/.test(l))
        .slice(0, 50);
      // Store errors in artifacts for implementation retry
      ctx.artifacts.compilationErrors = errors.length ? errors : [output.slice(0, 2000)];
      ctx.artifacts.compilerOutput = output.slice(0, 10000); // Store full output for UI display
      return {
        passed: false,
        errors: ctx.artifacts.compilationErrors,
        artifacts: {
          compilationErrors: ctx.artifacts.compilationErrors,
          compilerOutput: output.slice(0, 10000),
        },
      };
    } catch (err) {
      return {
        passed: false,
        errors: [`compiler spawn failed: ${err instanceof Error ? err.message : String(err)}`],
      };
    }
  },
  async onFail(ctx, result) {
    if (result.errors?.length) {
      try {
        await repairCode(ctx, result.errors);
      } catch (err) {
        ctx.log(
          `[compiler] code-repair failed: ${err instanceof Error ? err.message : String(err)}`,
          "warn"
        );
      }
    }
  },
};
