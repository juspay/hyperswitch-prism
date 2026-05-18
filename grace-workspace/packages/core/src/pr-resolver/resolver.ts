import { runClaudeCode } from "../tools/claude-code-runner.js";
import { emitPrResolverEvent } from "./events.js";
import { renderPrompt } from "./prompts.js";
import type { SubTask } from "./types.js";

/**
 * Per-sub-task Claude session. The "sub-task" is "all triggered review
 * comments on one connector in one PR". One Claude conversation handles
 * the initial fix attempt (`resolve-comment.md`) and any subsequent
 * cargo build/clippy retries (`fix-loop.md`) via `--resume`.
 *
 * Implementation note: we use Byne's existing `runClaudeCode` runner so we
 * inherit the configured `claude` CLI auth and Phase-12 session persistence.
 * No new SDK dependency.
 */

export interface RunResolverInput {
  subTask: SubTask;
  /** Absolute path to the working git clone where Claude makes edits. */
  worktreePath: string;
  /** Optional override for the prompts directory; falls back to config. */
  promptsDir?: string;
  /** Optional model override for this call. */
  claudeModel?: string;
  /** Optional hard timeout in ms; falls back to claudeCode.timeoutMs. */
  timeoutMs?: number;
}

export interface ResolverSessionOutput {
  summary: string;
  sessionId: string;
}

export interface RunFixLoopInput extends RunResolverInput {
  /** Session id captured from the first resolve call. */
  claudeSessionId: string;
  /** 1-based attempt number, displayed back to Claude in the prompt. */
  loopIteration: number;
  /** Concatenated cargo build + clippy error output. */
  errorOutput: string;
}

function buildThreadView(subTask: SubTask) {
  return subTask.threads.map((t, i) => {
    const trigger = (t.instruction || "").trim();
    const original = (t.originalCommentBody || "").trim();
    // Avoid showing the same body twice when the reviewer triggered the bot
    // in their first comment.
    const hasOriginal = !!original && original !== trigger;
    return {
      number: String(i + 1),
      path: t.path,
      line: t.line === null ? "?" : String(t.line),
      author: t.author,
      instruction: trigger,
      original_author: t.originalAuthor,
      original_comment: original,
      has_original: hasOriginal,
      thread_transcript: t.threadTranscript,
      diff_hunk: t.diffHunk ? t.diffHunk : undefined,
      is_outdated: t.isOutdated,
    };
  });
}

/**
 * First call for a sub-task. Sends the full resolve-comment prompt and
 * captures the session id for any subsequent fix-loop retries.
 */
export async function runResolverSession(
  input: RunResolverInput
): Promise<ResolverSessionOutput> {
  const threadView = buildThreadView(input.subTask);
  const rendered = renderPrompt(
    "resolve-comment",
    {
      connector: input.subTask.connector,
      thread_count: threadView.length,
      threads: threadView,
    },
    input.promptsDir
  );

  const sessionLabel = `pr-resolver-pr${input.subTask.prNumber}-${input.subTask.connector}`;
  emitPrResolverEvent("subtask_start", {
    pr: input.subTask.prNumber,
    connector: input.subTask.connector,
    threadCount: threadView.length,
    promptChars: rendered.length,
  });

  const { result, sessionId } = await runClaudeCode<string>({
    skillBody: rendered,
    userPayload: "",
    cwd: input.worktreePath,
    label: `pr-resolver-resolve-${input.subTask.connector}-pr${input.subTask.prNumber}`,
    sessionLabel,
    model: input.claudeModel,
    timeoutMs: input.timeoutMs,
    rawText: true,
    allowWrite: true,
    onStdoutLine: (line) => {
      emitPrResolverEvent("resolver_stream", {
        pr: input.subTask.prNumber,
        connector: input.subTask.connector,
        line,
      });
    },
  });

  emitPrResolverEvent("resolver_done", {
    pr: input.subTask.prNumber,
    connector: input.subTask.connector,
    summaryChars: result.length,
    sessionId,
  });

  return { summary: result, sessionId };
}

/**
 * Resume the sub-task's Claude session with the new cargo error output. Uses
 * `incremental: true` so Byne's runner skips the standard skill-body framing
 * and sends just the rendered fix-loop body — Claude already remembers the
 * original context from the first call.
 */
export async function runFixLoopSession(
  input: RunFixLoopInput
): Promise<ResolverSessionOutput> {
  const threadView = buildThreadView(input.subTask);
  const rendered = renderPrompt(
    "fix-loop",
    {
      connector: input.subTask.connector,
      loop_iteration: String(input.loopIteration),
      threads: threadView,
      error_output: input.errorOutput,
    },
    input.promptsDir
  );

  const sessionLabel = `pr-resolver-pr${input.subTask.prNumber}-${input.subTask.connector}`;
  emitPrResolverEvent("subtask_gate", {
    pr: input.subTask.prNumber,
    connector: input.subTask.connector,
    gate: "fix-loop-start",
    iteration: input.loopIteration,
  });

  const { result, sessionId } = await runClaudeCode<string>({
    skillBody: "",
    userPayload: rendered,
    cwd: input.worktreePath,
    label: `pr-resolver-fix-${input.subTask.connector}-pr${input.subTask.prNumber}-iter${input.loopIteration}`,
    sessionLabel,
    model: input.claudeModel,
    timeoutMs: input.timeoutMs,
    rawText: true,
    allowWrite: true,
    claudeSessionId: input.claudeSessionId,
    incremental: true,
    onStdoutLine: (line) => {
      emitPrResolverEvent("resolver_stream", {
        pr: input.subTask.prNumber,
        connector: input.subTask.connector,
        line,
        phase: `fix-loop-${input.loopIteration}`,
      });
    },
  });

  return { summary: result, sessionId };
}
