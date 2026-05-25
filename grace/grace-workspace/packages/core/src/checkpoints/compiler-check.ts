import { spawn } from "node:child_process";
import type { Checkpoint } from "../types.js";
import { runAI } from "../tools/runner-factory.js";

/**
 * Compiler Check Checkpoint - Phase 12 build-fix loop owner.
 *
 * Replaces the prior single-Claude-call design with a TypeScript-driven loop:
 *   1. Run cargo build directly (no agent needed).
 *   2. On failure, extract errors and `claude --resume <implementationSessionId>`
 *      with the errors as a short incremental message. The implementation
 *      Claude already remembers the L3 spec and the files it wrote, so the
 *      message body is ~1-2 KB instead of the ~30-50 KB of a fresh first-call
 *      payload.
 *   3. Re-run cargo build.
 *   4. Loop up to MAX_FIX_ITERATIONS.
 *
 * `retryFrom` is now self — the inner fix loop replaces engine-level rollback
 * to `implementation`. The engine's outer retry path becomes a manual-only
 * escape hatch if the inner loop runs out of iterations.
 */

const MAX_FIX_ITERATIONS = 5;
const CARGO_BUILD_TIMEOUT_MS = 10 * 60 * 1000; // 10 min per build attempt
const FIX_AI_TIMEOUT_MS = 15 * 60 * 1000; // 15 min per fix attempt

interface BuildResult {
  passed: boolean;
  output: string;
}

async function runCargoBuild(
  projectRoot: string,
  log: (msg: string, level: "info" | "warn" | "error" | "success") => void
): Promise<BuildResult> {
  return new Promise((resolve) => {
    const child = spawn(
      "cargo",
      ["build", "--package", "connector-integration"],
      { cwd: projectRoot, stdio: ["ignore", "pipe", "pipe"] }
    );

    const chunks: Buffer[] = [];
    child.stdout!.on("data", (c: Buffer) => {
      chunks.push(c);
      process.stdout.write(c);
    });
    child.stderr!.on("data", (c: Buffer) => {
      chunks.push(c);
      process.stderr.write(c);
    });

    const timer = setTimeout(() => {
      child.kill("SIGTERM");
      setTimeout(() => child.kill("SIGKILL"), 5000).unref();
    }, CARGO_BUILD_TIMEOUT_MS);
    timer.unref();

    child.on("error", (err) => {
      clearTimeout(timer);
      log(`cargo spawn failed: ${err.message}`, "error");
      resolve({ passed: false, output: `spawn error: ${err.message}` });
    });
    child.on("exit", (code) => {
      clearTimeout(timer);
      const output = Buffer.concat(chunks).toString("utf-8");
      resolve({ passed: code === 0, output });
    });
  });
}

/**
 * Pull the high-signal error lines out of a cargo build log. Includes
 * `error[E0NNN]` lines and the immediate `--> file:line:col` context that
 * follows them, plus any `error: could not compile` finals. Caps the
 * extracted output so the resume prompt stays small.
 */
function extractCargoErrors(output: string): string[] {
  const lines = output.split(/\r?\n/);
  const errors: string[] = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!;
    if (/^error(\[E\d+\])?:/.test(line) || /^error: /.test(line)) {
      // Capture up to 8 lines following the error for context (location + snippet)
      const block = [line];
      for (let j = 1; j <= 8 && i + j < lines.length; j++) {
        const next = lines[i + j]!;
        if (/^error(\[E\d+\])?:/.test(next) || /^warning:/.test(next)) break;
        block.push(next);
      }
      errors.push(block.join("\n"));
      if (errors.length >= 20) break; // cap to avoid blowing the prompt
    }
  }
  return errors;
}

function buildFixMessage(errors: string[], iteration: number): string {
  return [
    `Fix iteration ${iteration} / ${MAX_FIX_ITERATIONS}: cargo build failed with the following errors. Fix them and reply when you're done. No JSON output needed — just edit the files and confirm.`,
    "",
    "## Errors",
    "",
    "```",
    errors.join("\n\n"),
    "```",
    "",
    "Use your Read tool to inspect the affected files in their CURRENT on-disk state before editing (your in-memory model may be stale after the prior build attempt). Don't rewrite files that aren't mentioned in these errors. Reply with one short line summarizing what you changed.",
  ].join("\n");
}

export const compilerCheckCheckpoint: Checkpoint = {
  id: "compiler_check",
  name: "Compiler Check",
  description:
    "Run cargo build; on failure, resume implementation's Claude session with the errors and loop until passing (Phase 12 build-fix loop).",
  // Phase 12: self-loop. The inner build-fix iteration replaces engine-level
  // rollback to implementation.
  retryFrom: "compiler_check",
  // Generous outer wrapper: MAX_FIX_ITERATIONS × (build + fix) plus headroom.
  timeout: MAX_FIX_ITERATIONS * (CARGO_BUILD_TIMEOUT_MS + FIX_AI_TIMEOUT_MS) + 5 * 60_000,

  async run(ctx) {
    const task = ctx.artifacts.task;
    const connector = task?.targetConnectors?.[0];
    const flow = task?.paymentMethod || "Unknown";
    const projectRoot = task?.projectRoot;

    if (!projectRoot) {
      return { passed: false, errors: ["Missing project root"] };
    }

    ctx.log("[compiler_check] ╔═══════════════════════════════════════════════════════════╗", "info");
    ctx.log("[compiler_check] ║  Compiler Check — build-fix loop (Phase 12)               ║", "info");
    ctx.log("[compiler_check] ╚═══════════════════════════════════════════════════════════╝", "info");
    ctx.log(`[compiler_check] Connector: ${connector} · Flow: ${flow}`, "info");

    const implSessionId = ctx.artifacts.implementationSessionId;
    if (!implSessionId) {
      ctx.log(
        "[compiler_check] No implementationSessionId on artifacts — cannot drive build-fix loop. " +
          "Falling back to a single cargo build with no auto-fix. Run `implementation` first.",
        "warn"
      );
    }

    let lastBuildOutput = "";
    let iter = 0;

    while (iter <= MAX_FIX_ITERATIONS) {
      ctx.log(
        `[compiler_check] Build attempt ${iter + 1}/${MAX_FIX_ITERATIONS + 1}…`,
        "info"
      );
      const build = await runCargoBuild(projectRoot, ctx.log);
      lastBuildOutput = build.output;

      if (build.passed) {
        ctx.log("[compiler_check] ✓ cargo build succeeded", "success");
        return {
          passed: true,
          artifacts: {
            compilerCheck: { status: "SUCCESS", build_output: build.output.slice(-4000) },
            buildOutput: build.output.slice(-4000),
            compilerCheckIterations: iter,
            // Clear stale compilationErrors so a later checkpoint failure
            // doesn't reuse them.
            compilationErrors: [] as string[],
          },
        };
      }

      const errors = extractCargoErrors(build.output);
      ctx.log(
        `[compiler_check] ✗ cargo build failed (${errors.length} error block(s))`,
        "error"
      );
      // Stash for downstream/manual retry.
      ctx.artifacts.compilationErrors = errors;
      ctx.artifacts.compilerOutput = build.output.slice(-4000);

      if (iter === MAX_FIX_ITERATIONS) {
        ctx.log(
          `[compiler_check] Reached max fix iterations (${MAX_FIX_ITERATIONS}); giving up.`,
          "error"
        );
        return {
          passed: false,
          errors: [
            `Build still failing after ${MAX_FIX_ITERATIONS} fix attempts`,
            ...errors.slice(0, 3),
          ],
          artifacts: {
            compilerCheck: { status: "FAILED", build_output: build.output.slice(-4000) },
            buildOutput: build.output.slice(-4000),
            compilerCheckIterations: iter,
            compilationErrors: errors,
          },
        };
      }

      if (!implSessionId) {
        // We don't have an implementation session to resume — bail out
        // rather than fresh-spawning a Claude that doesn't know the L3 spec.
        return {
          passed: false,
          errors: [
            "Build failed and no implementation session is available to drive auto-fix. " +
              "Run the `implementation` checkpoint first, or use the dashboard's Re-run from implementation flow.",
            ...errors.slice(0, 3),
          ],
          artifacts: {
            compilerCheck: { status: "FAILED", build_output: build.output.slice(-4000) },
            buildOutput: build.output.slice(-4000),
            compilationErrors: errors,
          },
        };
      }

      iter++;
      ctx.log(
        `[compiler_check] Asking implementation Claude (session ${implSessionId.slice(0, 8)}…) to fix iteration ${iter}/${MAX_FIX_ITERATIONS}…`,
        "warn"
      );
      try {
        await runAI({
          claudeSessionId: implSessionId,
          incremental: true,
          userPayload: buildFixMessage(errors, iter),
          skillBody: "",
          rawText: true, // we don't need JSON back — just the file edits
          cwd: projectRoot,
          label: `compiler_check:fix-iter-${iter}`,
          timeoutMs: FIX_AI_TIMEOUT_MS,
          allowWrite: true, // codegen must Edit files to fix
        });
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        ctx.log(`[compiler_check] Fix iteration ${iter} threw: ${msg}`, "error");
        return {
          passed: false,
          errors: [
            `Fix iteration ${iter} failed: ${msg}`,
            ...errors.slice(0, 3),
          ],
          artifacts: {
            compilerCheck: { status: "FAILED", build_output: lastBuildOutput.slice(-4000) },
            buildOutput: lastBuildOutput.slice(-4000),
            compilationErrors: errors,
          },
        };
      }
    }

    // Should be unreachable — the loop returns on success or at iteration cap.
    return {
      passed: false,
      errors: ["compiler_check: unexpected loop exit"],
    };
  },
};
