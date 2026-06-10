import { execa } from "execa";
import { emitPrResolverEvent } from "./events.js";
import { runFixLoopSession } from "./resolver.js";
import type { SubTask } from "./types.js";

/**
 * Build / clippy / fix loop. Sits between the resolver's first Claude call
 * and the final push. The flow is always:
 *   cargo build → cargo clippy → if either fails, resume the Claude session
 *   with the error output, then try again. Up to `maxLoops` fix attempts.
 *
 * Build/clippy commands are configurable (see `prResolver.cargoBuild` and
 * `cargoClippy` in config.yml) so non-default crates can be wired in
 * without touching this file.
 */

const MAX_ERROR_OUTPUT_CHARS = 30_000;
const DEFAULT_CARGO_TIMEOUT_MS = 30 * 60 * 1000;

export interface CargoLoopInput {
  subTask: SubTask;
  worktreePath: string;
  /** Session id from the first `runResolverSession` call. */
  claudeSessionId: string;
  buildCommand: { command: string; args: string[] };
  clippyCommand: { command: string; args: string[] };
  /** Max fix iterations. Each iteration is one Claude resume + one rebuild. */
  maxLoops: number;
  promptsDir?: string;
  claudeModel?: string;
  /** Hard timeout for each Claude fix call. */
  fixTimeoutMs?: number;
  /** Hard timeout per cargo invocation. Defaults to 30 min. */
  cargoTimeoutMs?: number;
}

export interface CargoLoopResult {
  buildPassed: boolean;
  clippyPassed: boolean;
  loopCount: number;
  /** Last session id observed — write back to state so future cycles can reuse it if useful. */
  finalSessionId: string;
  /** Trailing error output when we exit with `buildPassed=false || clippyPassed=false`. */
  errorOutput: string;
}

interface CargoResult {
  passed: boolean;
  output: string;
}

async function runCargo(
  spec: { command: string; args: string[] },
  cwd: string,
  timeoutMs: number
): Promise<CargoResult> {
  const result = await execa(spec.command, spec.args, {
    cwd,
    reject: false,
    timeout: timeoutMs,
    all: true,
  });
  // Cargo writes errors to stderr and progress to stdout; `all: true` merges
  // them in chronological order. Tail the last 30KB so a noisy build can't
  // blow the LLM context.
  const combined = result.all ?? `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
  const truncated =
    combined.length > MAX_ERROR_OUTPUT_CHARS
      ? `... (truncated ${combined.length - MAX_ERROR_OUTPUT_CHARS} chars) ...\n` +
        combined.slice(-MAX_ERROR_OUTPUT_CHARS)
      : combined;
  const timedOut = result.timedOut === true;
  const banner = timedOut
    ? `[CARGO TIMED OUT after ${Math.round(timeoutMs / 1000)}s — bump prResolver.cargoTimeoutMs if your machine needs longer]\n\n`
    : "";
  return {
    passed: result.exitCode === 0 && !timedOut,
    output: banner + truncated,
  };
}

export async function runCargoFixLoop(
  input: CargoLoopInput
): Promise<CargoLoopResult> {
  let sessionId = input.claudeSessionId;
  let loopCount = 0;
  let lastErrorOutput = "";

  while (true) {
    // ─── cargo build ───────────────────────────────────────────────────
    emitPrResolverEvent("build_start", {
      pr: input.subTask.prNumber,
      connector: input.subTask.connector,
      iteration: loopCount,
    });
    const build = await runCargo(
      input.buildCommand,
      input.worktreePath,
      input.cargoTimeoutMs ?? DEFAULT_CARGO_TIMEOUT_MS
    );
    if (!build.passed) {
      emitPrResolverEvent("build_fail", {
        pr: input.subTask.prNumber,
        connector: input.subTask.connector,
        iteration: loopCount,
        outputTail: build.output.slice(-2000),
      });
      lastErrorOutput = build.output;
      if (loopCount >= input.maxLoops) {
        return {
          buildPassed: false,
          clippyPassed: false,
          loopCount,
          finalSessionId: sessionId,
          errorOutput: lastErrorOutput,
        };
      }
      loopCount += 1;
      const fix = await runFixLoopSession({
        subTask: input.subTask,
        worktreePath: input.worktreePath,
        claudeSessionId: sessionId,
        loopIteration: loopCount,
        errorOutput: build.output,
        promptsDir: input.promptsDir,
        claudeModel: input.claudeModel,
        timeoutMs: input.fixTimeoutMs,
      });
      sessionId = fix.sessionId;
      continue;
    }
    emitPrResolverEvent("build_pass", {
      pr: input.subTask.prNumber,
      connector: input.subTask.connector,
      iteration: loopCount,
    });

    // ─── cargo clippy ──────────────────────────────────────────────────
    const clippy = await runCargo(
      input.clippyCommand,
      input.worktreePath,
      input.cargoTimeoutMs ?? DEFAULT_CARGO_TIMEOUT_MS
    );
    if (!clippy.passed) {
      emitPrResolverEvent("clippy_fail", {
        pr: input.subTask.prNumber,
        connector: input.subTask.connector,
        iteration: loopCount,
        outputTail: clippy.output.slice(-2000),
      });
      lastErrorOutput = clippy.output;
      if (loopCount >= input.maxLoops) {
        return {
          buildPassed: true,
          clippyPassed: false,
          loopCount,
          finalSessionId: sessionId,
          errorOutput: lastErrorOutput,
        };
      }
      loopCount += 1;
      const fix = await runFixLoopSession({
        subTask: input.subTask,
        worktreePath: input.worktreePath,
        claudeSessionId: sessionId,
        loopIteration: loopCount,
        errorOutput: clippy.output,
        promptsDir: input.promptsDir,
        claudeModel: input.claudeModel,
        timeoutMs: input.fixTimeoutMs,
      });
      sessionId = fix.sessionId;
      continue;
    }
    emitPrResolverEvent("clippy_pass", {
      pr: input.subTask.prNumber,
      connector: input.subTask.connector,
      iteration: loopCount,
    });

    return {
      buildPassed: true,
      clippyPassed: true,
      loopCount,
      finalSessionId: sessionId,
      errorOutput: "",
    };
  }
}
