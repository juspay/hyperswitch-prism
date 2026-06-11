import { getConfig, type RunnerType } from "../config.js";
import {
  runOpencode,
  checkOpencodeHealth,
  type OpencodeRunOptions,
  type OpencodeHealthResult,
} from "./opencode-runner.js";
import {
  runClaudeCode,
  checkClaudeCodeHealth,
  type ClaudeCodeRunOptions,
  type ClaudeCodeHealthResult,
} from "./claude-code-runner.js";

/**
 * Unified options for running AI tasks - compatible with both OpenCode and Claude Code.
 * This is a superset of both OpencodeRunOptions and ClaudeCodeRunOptions.
 */
export interface AIRunOptions {
  /** The skill body / system prompt — describes what to produce. */
  skillBody: string;
  /** Structured user payload (JSON-serializable). */
  userPayload: unknown;
  /** Working directory the runner should run in (the target repo root). */
  cwd: string;
  /** Short label for logs. */
  label: string;
  /** Optional model override. */
  model?: string;
  /** Hard timeout in ms. Default 10 min. */
  timeoutMs?: number;
  /**
   * If true, the answer is treated as raw text and returned as a string
   * instead of JSON.parse'd. Use for research / summary agents that return
   * markdown or plain prose.
   */
  rawText?: boolean;
  /** Additional CLI arguments to pass to the runner. */
  extraArgs?: string[];
  /**
   * If true, the agent is allowed to use all tools including write and edit.
   * Default is false (read-only tools).
   */
  allowWrite?: boolean;
  /**
   * Phase 12: resume a prior Claude conversation by session id. Only honored
   * by the claude-code runner; opencode runner will throw if set (opencode has
   * no equivalent persistent-session concept in this codebase).
   */
  claudeSessionId?: string;
  /**
   * Phase 12: pre-picked UUID for the new (non-resume) call. Lets callers
   * decide the session id in advance — useful when spawning sibling sessions
   * that need to know each other's ids up front. Claude-code only.
   */
  preferredSessionId?: string;
  /**
   * Phase 12: treat `userPayload` as the entire prompt body (typically a
   * short follow-up message) and skip the standard skill body / `## Input` /
   * answer-instructions framing. Required when `claudeSessionId` is set.
   * Claude-code only.
   */
  incremental?: boolean;
  /**
   * Phase 15: human-readable label for the Claude conversation (e.g.
   * `stripe-card3ds-implementation`). Logged in the spawn line for
   * grep-ability; does not affect spawn args. Claude-code only.
   */
  sessionLabel?: string;
  /**
   * If set, invoked once per ANSI-stripped non-empty stdout line from the
   * underlying runner. Checkpoints typically wire this to `ctx.log` so the
   * dashboard sees each tool use (Read /foo, Edit /bar, Bash cargo check)
   * as it happens, instead of just a spinner. Works for both claude-code
   * (rendered from stream-json) and opencode (raw lines).
   */
  onStdoutLine?: (line: string) => void;
}

/**
 * Unified health result type - compatible with both runner health results.
 */
export interface AIHealthResult {
  connected: boolean;
  /** The runner type that was checked. */
  runner: RunnerType;
  /** URL or version info depending on runner type. */
  connectionInfo?: string;
  error?: string;
  latencyMs?: number;
}

/**
 * Phase 12: unified result shape. Every runAI call returns both the parsed
 * result and the Claude session id that produced it. Callers that don't need
 * the session id can `const { result } = await runAI(...)` and ignore the
 * rest. Callers that DO need it (per-phase persistence) read `sessionId` and
 * stash it on ctx.artifacts for later --resume invocations.
 *
 * The opencode runner has no persistent-session concept, so its sessionId is
 * a single-use throwaway UUID returned only to keep the type uniform — it
 * cannot be passed back as `claudeSessionId` (the opencode branch rejects
 * that option).
 */
export interface AIRunResult<T> {
  result: T;
  sessionId: string;
}

/**
 * Interface that both runners must implement.
 */
export interface RunnerInterface {
  run<T>(opts: AIRunOptions): Promise<AIRunResult<T>>;
  checkHealth(): Promise<AIHealthResult>;
}

/**
 * Get the currently configured runner type.
 */
export function getConfiguredRunner(): RunnerType {
  return getConfig().runner ?? "opencode";
}

/**
 * Check if the configured runner is healthy.
 * This is a unified health check that works with both OpenCode and Claude Code.
 */
export async function checkAIHealth(): Promise<AIHealthResult> {
  const runner = getConfiguredRunner();

  if (runner === "claude-code") {
    const result = await checkClaudeCodeHealth();
    return {
      connected: result.connected,
      runner: "claude-code",
      connectionInfo: result.version,
      error: result.error,
      latencyMs: result.latencyMs,
    };
  } else {
    const result = await checkOpencodeHealth();
    return {
      connected: result.connected,
      runner: "opencode",
      connectionInfo: result.url,
      error: result.error,
      latencyMs: result.latencyMs,
    };
  }
}

/**
 * Run an AI task using the configured runner.
 * This is the main entry point for checkpoint code - it automatically
 * delegates to the appropriate runner based on configuration.
 *
 * Phase 12: returns `{ result, sessionId }`. Callers that don't care about
 * persistence can `const { result } = await runAI(...)`. Callers that do
 * want to resume on retry stash `sessionId` on `ctx.artifacts` and pass it
 * back as `claudeSessionId` next time.
 */
export async function runAI<T = unknown>(
  opts: AIRunOptions
): Promise<AIRunResult<T>> {
  const runner = getConfiguredRunner();

  if (runner === "claude-code") {
    const ccOpts: ClaudeCodeRunOptions = {
      skillBody: opts.skillBody,
      userPayload: opts.userPayload,
      cwd: opts.cwd,
      label: opts.label,
      model: opts.model,
      timeoutMs: opts.timeoutMs,
      rawText: opts.rawText,
      extraArgs: opts.extraArgs,
      allowWrite: opts.allowWrite,
      claudeSessionId: opts.claudeSessionId,
      preferredSessionId: opts.preferredSessionId,
      incremental: opts.incremental,
      sessionLabel: opts.sessionLabel,
      onStdoutLine: opts.onStdoutLine,
    };
    return runClaudeCode<T>(ccOpts);
  } else {
    // Opencode path doesn't support persistent sessions — fail loud if a
    // caller tries to use them with the wrong runner. Better than silently
    // ignoring and pretending to resume.
    if (opts.claudeSessionId || opts.incremental) {
      throw new Error(
        `runAI: claudeSessionId/incremental are claude-code only (current runner: ${runner}, label: ${opts.label})`
      );
    }
    const ocOpts: OpencodeRunOptions = {
      skillBody: opts.skillBody,
      userPayload: opts.userPayload,
      cwd: opts.cwd,
      label: opts.label,
      model: opts.model,
      timeoutMs: opts.timeoutMs,
      rawText: opts.rawText,
      onStdoutLine: opts.onStdoutLine,
    };
    const result = await runOpencode<T>(ocOpts);
    // Synthetic single-use id keeps the return shape uniform. Storing it
    // on artifacts won't enable resume (opencode would reject it next time),
    // so callers in opencode mode should treat sessionId as a no-op.
    return { result, sessionId: opts.preferredSessionId ?? "opencode-noop" };
  }
}

/**
 * Temporarily override the runner for a single operation.
 * This is useful when the task specifies a different runner than the config.
 *
 * Usage:
 *   const result = await withRunner("claude-code", () => runAI({ ... }));
 */
export async function withRunner<T>(
  runner: RunnerType,
  fn: () => Promise<T>
): Promise<T> {
  const original = getConfig().runner;
  try {
    getConfig().runner = runner;
    return await fn();
  } finally {
    getConfig().runner = original;
  }
}
