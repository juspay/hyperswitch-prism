import { spawn } from "node:child_process";
import { promises as fs } from "node:fs";
import { randomUUID } from "node:crypto";
import path from "node:path";
import { getConfig } from "../config.js";

/**
 * Shells out to the local `claude` CLI to run a planning step.
 *
 * This runner provides equivalent functionality to opencode-runner but for
 * Anthropic's Claude Code CLI. It spawns the `claude` command with appropriate
 * flags and captures the output for downstream processing.
 *
 * To avoid brittle stdout parsing we instruct the prompt to reply with ONLY
 * the raw JSON in the chat message. Claude's stdout is captured for progress
 * logging and the final answer is extracted.
 */

export interface ClaudeCodeRunOptions {
  /** The skill body / system prompt — describes what to produce. */
  skillBody: string;
  /** Structured user payload (JSON-serializable). */
  userPayload: unknown;
  /** Working directory claude should run in (the target repo root). */
  cwd: string;
  /** Short label for logs. */
  label: string;
  /** Optional model override, e.g. "claude-sonnet-4-6". */
  model?: string;
  /** Hard timeout in ms. Default 10 min. */
  timeoutMs?: number;
  /**
   * If true, the answer is treated as raw text and returned as a string
   * instead of JSON.parse'd. Use for research / summary agents that return
   * markdown or plain prose.
   */
  rawText?: boolean;
  /** Additional CLI arguments to pass to claude. */
  extraArgs?: string[];
  /**
   * If true, the agent is allowed to use all tools including write and edit.
   * Default is false (read-only tools).
   */
  allowWrite?: boolean;
  /**
   * If set, the runner does `claude --resume <id>` instead of starting a fresh
   * session. The caller is responsible for having stored this id from a prior
   * `runClaudeCode` call. Combine with `incremental: true` to send a short
   * follow-up message instead of the full skill body + payload framing.
   */
  claudeSessionId?: string;
  /**
   * Pre-picked UUID for a new (non-resume) call. If omitted and
   * `claudeSessionId` is not set, the runner generates a fresh UUID via
   * `randomUUID()`. Use this when the caller wants the session id to be
   * predictable or shared between siblings.
   */
  preferredSessionId?: string;
  /**
   * If true, the runner treats `userPayload` as the full prompt body (either a
   * string or stringified JSON) and skips the standard skill body / `## Input`
   * / `## How to return your answer` framing. Intended for resume calls that
   * carry a short incremental follow-up message ("here are the errors, fix").
   *
   * The caller is responsible for putting any answer-shape instructions
   * (e.g. "reply with JSON") into the message body when needed. The runner's
   * stdout parsing is still controlled by `rawText`.
   */
  incremental?: boolean;
  /**
   * Phase 15: human-readable identifier for the conversation thread (e.g.
   * `stripe-card3ds-implementation`). Logged alongside the uuid in the spawn
   * line so `grep stripe-card3ds-implementation engine.log` pulls every
   * relevant claude invocation. Does NOT affect the spawn args.
   */
  sessionLabel?: string;
  /**
   * If set, invoked once per line of stdout from `claude --verbose`. Used
   * by the PR Resolver to stream live progress through to the dashboard.
   * The line is ANSI-stripped before the callback fires; the underlying
   * stdout capture for the final result is unchanged.
   */
  onStdoutLine?: (line: string) => void;
}

function dbg(...args: unknown[]) {
  // eslint-disable-next-line no-console
  console.error("\x1b[90m[claude-code]", ...args, "\x1b[0m");
}

/**
 * Parse one line of `claude --output-format stream-json` into 0+ short,
 * human-readable progress lines. Each tool use, tool result, and text
 * chunk becomes one rendered line — designed for the PR Resolver's "Claude
 * live output" panel so a reviewer can watch what Claude is doing in real
 * time without parsing the raw JSONL themselves.
 *
 * On `type: "result"`, captures the final assistant text via `onFinalText`
 * so the runner returns the same string it would have under plain-text
 * mode. Defensive: a non-JSON line passes through unchanged.
 */
function renderStreamJsonLine(
  line: string,
  onFinalText: (text: string) => void
): string[] {
  let evt: Record<string, unknown>;
  try {
    evt = JSON.parse(line) as Record<string, unknown>;
  } catch {
    return [line];
  }
  const type = String(evt.type ?? "");
  const out: string[] = [];

  if (type === "system") {
    if (evt.subtype === "init") {
      const sid = String(evt.session_id ?? "");
      const model = String(evt.model ?? "?");
      out.push(`▶ session ${sid.slice(0, 8)} · model ${model}`);
    }
    return out;
  }

  if (type === "assistant") {
    const message = evt.message as Record<string, unknown> | undefined;
    const content = (message?.content as Array<Record<string, unknown>>) ?? [];
    for (const block of content) {
      const blockType = String(block.type ?? "");
      if (blockType === "text") {
        const text = String(block.text ?? "").trim();
        if (!text) continue;
        const snippet = text.length > 240 ? text.slice(0, 240) + "…" : text;
        out.push(`💬 ${snippet}`);
      } else if (blockType === "thinking") {
        const text = String(block.thinking ?? "").trim();
        if (!text) continue;
        const snippet = text.length > 200 ? text.slice(0, 200) + "…" : text;
        out.push(`🤔 ${snippet}`);
      } else if (blockType === "tool_use") {
        const name = String(block.name ?? "?");
        const input = (block.input as Record<string, unknown>) ?? {};
        out.push(`🔧 ${name}(${summarizeToolInput(name, input)})`);
      }
    }
    return out;
  }

  if (type === "user") {
    const message = evt.message as Record<string, unknown> | undefined;
    const content = (message?.content as Array<Record<string, unknown>>) ?? [];
    for (const block of content) {
      if (block.type !== "tool_result") continue;
      const isError = block.is_error === true;
      let text = "";
      if (typeof block.content === "string") {
        text = block.content;
      } else if (Array.isArray(block.content)) {
        text = (block.content as Array<Record<string, unknown>>)
          .map((c) => String(c.text ?? ""))
          .join("");
      }
      const firstLine = text.split("\n", 1)[0]?.trim() ?? "";
      const snippet =
        firstLine.length > 120 ? firstLine.slice(0, 120) + "…" : firstLine;
      out.push(isError ? `   ✗ ${snippet || "(error)"}` : `   ✓ ${snippet || "(ok)"}`);
    }
    return out;
  }

  if (type === "result") {
    // Final assistant text + cost. Capture for the runner's return value.
    const text = typeof evt.result === "string" ? (evt.result as string) : "";
    if (text) onFinalText(text);
    const usage = (evt.usage as Record<string, unknown>) ?? {};
    const inTok = Number(usage.input_tokens ?? 0);
    const outTok = Number(usage.output_tokens ?? 0);
    const dur = Number(evt.duration_ms ?? 0);
    out.push(
      `✓ done · ${Math.round(dur / 1000)}s · in=${inTok} out=${outTok} tokens`
    );
    return out;
  }

  // Unknown event type — pass the raw line through so we don't drop signal.
  return [line];
}

/**
 * Compact summary of a tool call's input for the live-output panel.
 * Keeps it under ~120 chars and prefers the most-useful field for each
 * known tool: file_path for Read/Edit/Write, pattern for Grep, command for
 * Bash, url for WebFetch, etc.
 */
function summarizeToolInput(
  name: string,
  input: Record<string, unknown>
): string {
  const candidates: string[] = [];
  if (typeof input.file_path === "string") candidates.push(input.file_path);
  if (typeof input.path === "string") candidates.push(input.path);
  if (typeof input.pattern === "string") candidates.push(input.pattern);
  if (typeof input.command === "string") candidates.push(input.command);
  if (typeof input.url === "string") candidates.push(input.url);
  if (typeof input.description === "string") candidates.push(input.description);
  if (typeof input.query === "string") candidates.push(input.query);
  if (name === "Edit" && typeof input.old_string === "string") {
    const first = input.old_string.split("\n", 1)[0] ?? "";
    candidates.push(`replace: ${first.trim().slice(0, 60)}`);
  }
  if (candidates.length === 0) {
    try {
      candidates.push(JSON.stringify(input));
    } catch {
      candidates.push("?");
    }
  }
  const joined = candidates[0]!;
  return joined.length > 120 ? joined.slice(0, 120) + "…" : joined;
}

export interface ClaudeCodeHealthResult {
  connected: boolean;
  version?: string;
  error?: string;
  latencyMs?: number;
}

/**
 * Check if the claude CLI is available and get its version.
 * Returns a result object with availability status and version info.
 */
export async function checkClaudeCodeHealth(): Promise<ClaudeCodeHealthResult> {
  const start = Date.now();

  return new Promise<ClaudeCodeHealthResult>((resolve) => {
    const child = spawn("claude", ["--version"], {
      stdio: ["ignore", "pipe", "pipe"],
      env: process.env,
    });

    let stdout = "";
    let stderr = "";

    child.stdout!.on("data", (chunk: Buffer) => {
      stdout += chunk.toString("utf-8");
    });

    child.stderr!.on("data", (chunk: Buffer) => {
      stderr += chunk.toString("utf-8");
    });

    const timer = setTimeout(() => {
      child.kill("SIGTERM");
      resolve({
        connected: false,
        error: "Timeout checking claude version (5s)",
        latencyMs: Date.now() - start,
      });
    }, 5000);

    child.on("error", (err) => {
      clearTimeout(timer);
      resolve({
        connected: false,
        error: err.message,
        latencyMs: Date.now() - start,
      });
    });

    child.on("exit", (code) => {
      clearTimeout(timer);
      const latencyMs = Date.now() - start;

      if (code === 0) {
        const version = stdout.trim() || stderr.trim();
        resolve({
          connected: true,
          version: version || "unknown",
          latencyMs,
        });
      } else {
        resolve({
          connected: false,
          error: `claude --version exited with code ${code}: ${stderr || stdout}`,
          latencyMs,
        });
      }
    });
  });
}

export async function runClaudeCode<T = unknown>(
  opts: ClaudeCodeRunOptions
): Promise<{ result: T; sessionId: string }> {
  const absCwdEarly = path.resolve(opts.cwd);

  if (opts.incremental && !opts.claudeSessionId) {
    throw new Error(
      `runClaudeCode: incremental=true requires claudeSessionId (label: ${opts.label})`
    );
  }

  // Determine the session id we'll bind to this call. Three precedence cases:
  //   1. Caller passed claudeSessionId → resume that session.
  //   2. Caller passed preferredSessionId → start fresh with that uuid.
  //   3. Neither → generate a fresh uuid here.
  // We always return whichever id was used, so callers can persist it for
  // later `--resume` invocations.
  const sessionId =
    opts.claudeSessionId ?? opts.preferredSessionId ?? randomUUID();
  const isResume = !!opts.claudeSessionId;

  // Prompt construction.
  //   - First-call (non-incremental): full skill body + JSON payload + answer
  //     instructions, as before.
  //   - Resume / incremental: the caller supplies the entire message body. We
  //     pass it through verbatim; any "reply with JSON" cue must already be in
  //     it. The runner's stdout parsing is still controlled by `rawText`.
  let prompt: string;
  if (opts.incremental) {
    prompt =
      typeof opts.userPayload === "string"
        ? opts.userPayload
        : JSON.stringify(opts.userPayload, null, 2);
  } else {
    const answerInstructions = opts.allowWrite
      ? [
          "## How to return your answer",
          "",
          "You have FULL TOOL ACCESS including Write and Edit. Use any tools necessary to complete your task.",
          "",
          opts.rawText
            ? "Reply with your final report as plain text (markdown is fine)."
            : "Reply with ONLY the raw JSON object in your chat message. No markdown fences, no thinking out loud, no preamble. The very first character of your reply must be `{` and the very last must be `}`.",
        ]
      : opts.rawText
        ? [
            "## How to return your answer",
            "",
            "IMPORTANT: Do NOT use your write or edit tools to create any files. Do NOT create answer.json or any other output files. Just reply in chat with your final report as plain text (markdown is fine).",
            "",
            "Use your read tools (Read, Grep, Glob, Bash) to gather evidence BEFORE replying. Then reply with the report directly in your message.",
          ]
        : [
            "## How to return your answer",
            "",
            "IMPORTANT: Do NOT use your write or edit tools to create any files. Do NOT create answer.json or any other output files. Do NOT write prose or explanation — reply with ONLY the raw JSON object in your chat message. No markdown fences, no thinking out loud, no preamble. The very first character of your reply must be `{` and the very last must be `}`.",
            "",
            "Use your read tools (Read, Grep, Glob, Bash) to gather evidence BEFORE replying. Then reply with ONLY the JSON.",
          ];

    prompt = [
      opts.skillBody.trim(),
      "",
      "## Input",
      "",
      "```json",
      JSON.stringify(opts.userPayload, null, 2),
      "```",
      "",
      ...answerInstructions,
    ].join("\n");
  }

  const absCwd = absCwdEarly;

  const cc = getConfig().claudeCode;
  // Caller override > config.yml > default. Env vars are escape hatches only.
  const model =
    opts.model ?? process.env.TENXGRACE_CLAUDE_CODE_MODEL ?? cc.model;

  const args: string[] = [];

  // Add model if specified
  if (model) {
    args.push("--model", model);
  }

  // When a caller passes `onStdoutLine`, switch to claude's streaming JSON
  // output so we see progressive events (tool uses, file reads, edits)
  // instead of just the final response dumped at the end of the run.
  // Without this, the PR Resolver's "Claude live output" panel sits empty
  // for ~50s and then shows the closing 3 lines as a single dump. With
  // stream-json, every Read/Edit/Bash/TodoWrite is broadcast in real time.
  // Requires `--print` (non-interactive) + `--verbose` (claude rejects
  // stream-json without verbose). The runner parses each line into a
  // human-readable progress message before forwarding to `onStdoutLine`.
  const streamJson = !!opts.onStdoutLine;
  if (streamJson) {
    args.push("--print");
    args.push("--output-format", "stream-json");
  }

  // Add verbose output flags
  args.push("--verbose");

  // Skip permission prompts for subagent - parent manages permissions
  args.push("--dangerously-skip-permissions");

  // Phase 12: per-phase Claude conversation continuity. Either resume an
  // existing session (caller stored the id from a prior call) or start a fresh
  // one with a pre-picked uuid we can return for future resumes.
  if (isResume) {
    args.push("--resume", sessionId);
  } else {
    args.push("--session-id", sessionId);
  }

  // Add any extra args from config or options
  const extraArgs = opts.extraArgs ?? cc.extraArgs ?? [];
  if (extraArgs.length > 0) {
    args.push(...extraArgs);
  }

  // Add the prompt as the final argument
  args.push(prompt);

  const timeoutMs = opts.timeoutMs ?? cc.timeoutMs ?? 10 * 60 * 1000;
  const timeoutEnabled = Number.isFinite(timeoutMs) && timeoutMs > 0;
  const startedAt = Date.now();
  const sessionMode = isResume ? `resume(${sessionId.slice(0, 8)}…)` : `new(${sessionId.slice(0, 8)}…)`;
  const friendlyTag = opts.sessionLabel ? ` · session=${opts.sessionLabel}` : "";
  const timeoutTag = timeoutEnabled ? ` · timeout=${Math.round(timeoutMs / 1000)}s` : " · timeout=disabled";
  // eslint-disable-next-line no-console
  console.log(
    `\x1b[36m[claude-code] → ${opts.label} · cwd=${absCwd} · model=${model ?? "default"} · ${sessionMode}${friendlyTag}${timeoutTag} · prompt=${prompt.length}ch\x1b[0m`
  );

  // Capture stdout and stderr to extract the model's answer and error details.
  const stdoutChunks: Buffer[] = [];
  const stderrChunks: Buffer[] = [];
  // In stream-json mode, the final assistant text comes from the `result`
  // event rather than concatenated stdout. Captured here so the post-run
  // parser uses it instead of the raw JSON stream.
  let streamFinalText: string | undefined;

  await new Promise<void>((resolve, reject) => {
    const child = spawn("claude", args, {
      cwd: absCwd,
      stdio: ["ignore", "pipe", "pipe"],
      env: process.env,
    });

    // Capture stdout into a buffer AND mirror it to process.stdout so the
    // user still sees claude's progress output in the engine logs. When the
    // caller passes `onStdoutLine`, also line-buffer the stream and fire
    // the callback per line so the PR Resolver can pipe live progress to
    // the dashboard.
    let stdoutLineBuffer = "";
    // eslint-disable-next-line no-control-regex
    const ansiRe = /\x1b\[[0-9;]*m/g;
    const emitLine = (line: string) => {
      try {
        opts.onStdoutLine?.(line);
      } catch {
        /* swallow — user callback shouldn't kill the run */
      }
    };
    child.stdout!.on("data", (chunk: Buffer) => {
      stdoutChunks.push(chunk);
      // In stream-json mode, the raw JSON dump is noisy in engine logs.
      // Mirror nothing here — the rendered progress lines below are echoed
      // to stdout from within `emitLine` (via the runner's own writer).
      if (!streamJson) {
        process.stdout.write(chunk);
      }
      if (opts.onStdoutLine) {
        stdoutLineBuffer += chunk.toString("utf8");
        let idx: number;
        while ((idx = stdoutLineBuffer.indexOf("\n")) >= 0) {
          const raw = stdoutLineBuffer.slice(0, idx);
          stdoutLineBuffer = stdoutLineBuffer.slice(idx + 1);
          const cleaned = raw.replace(ansiRe, "").replace(/\r$/, "");
          if (cleaned.trim().length === 0) continue;
          if (streamJson) {
            const rendered = renderStreamJsonLine(cleaned, (txt) => {
              streamFinalText = txt;
            });
            for (const r of rendered) {
              emitLine(r);
              // Tee the rendered progress to the engine console too — keeps
              // engine.log useful for grep'ing tool calls across runs.
              process.stdout.write(r + "\n");
            }
          } else {
            emitLine(cleaned);
          }
        }
      }
    });

    // Capture stderr for error diagnostics AND mirror to process.stderr.
    child.stderr!.on("data", (chunk: Buffer) => {
      stderrChunks.push(chunk);
      process.stderr.write(chunk);
    });

    const timer: NodeJS.Timeout | null = timeoutEnabled
      ? setTimeout(() => {
          dbg(`timeout after ${timeoutMs}ms — killing claude`);
          child.kill("SIGTERM");
          setTimeout(() => child.kill("SIGKILL"), 5000).unref();
          const stderr = Buffer.concat(stderrChunks).toString("utf-8").trim();
          const detail = stderr ? ` — stderr: ${stderr.slice(-500)}` : "";
          reject(new Error(`claude timed out after ${timeoutMs}ms (label: ${opts.label})${detail}`));
        }, timeoutMs)
      : null;
    timer?.unref();

    child.on("error", (err) => {
      if (timer) clearTimeout(timer);
      reject(new Error(`claude spawn failed: ${err.message}`));
    });
    child.on("exit", (code, signal) => {
      if (timer) clearTimeout(timer);
      const stderr = Buffer.concat(stderrChunks).toString("utf-8").trim();
      const detail = stderr ? ` — stderr: ${stderr.slice(-500)}` : "";
      if (signal) {
        reject(new Error(`claude killed by signal ${signal} (label: ${opts.label})${detail}`));
      } else if (code !== 0) {
        reject(new Error(`claude exited with code ${code} (label: ${opts.label})${detail}`));
      } else {
        resolve();
      }
    });
  });

  // Clean up stray files the model may have created (answer.json, answer.txt, etc.)
  const strayFiles = ["answer.json", "answer.txt"];
  for (const name of strayFiles) {
    const stray = path.join(absCwd, name);
    fs.rm(stray, { force: true }).catch(() => undefined);
  }
  // Also clean up temp directories if they exist
  fs.rm(path.join(absCwd, ".claude-tmp"), { recursive: true, force: true }).catch(() => undefined);

  const elapsed = Date.now() - startedAt;
  const raw = Buffer.concat(stdoutChunks).toString("utf-8");
  // eslint-disable-next-line no-console
  console.log(`\x1b[36m[claude-code] ← ${opts.label} · ${elapsed}ms · stdout=${raw.length}ch\x1b[0m`);

  if (!raw.trim()) {
    throw new Error(
      `claude finished but produced no output (label: ${opts.label}). stdout was empty.`
    );
  }

  // In stream-json mode the raw stdout is a JSONL dump; the final assistant
  // text lives in the `result` event. Fall back to raw if for some reason
  // the result event was missing (e.g. claude crashed before sending it).
  const sourceText =
    streamJson && streamFinalText !== undefined ? streamFinalText : raw;

  // Strip BOM and ANSI escape codes.
  // eslint-disable-next-line no-control-regex
  let cleaned = sourceText.trim().replace(/^﻿/, "").replace(/\x1b\[[0-9;]*m/g, "");

  if (opts.rawText) {
    // Drop claude chrome lines ("> build", "[0m", etc.)
    const lines = cleaned.split("\n");
    const contentStart = lines.findIndex(
      (l) => l.trim().length > 0 && !l.trim().startsWith(">") && !l.startsWith("[0m")
    );
    if (contentStart >= 0) {
      cleaned = lines.slice(contentStart).join("\n").trim();
    }
    return { result: cleaned as unknown as T, sessionId };
  }

  // JSON mode: find the last complete JSON object in the output.
  // The model's chat reply may include tool-call chatter before the final JSON.

  // First, try to extract JSON from markdown code fences if present
  // This handles cases where model wraps JSON in ```json ... ``` or ``` ... ```
  const codeFenceRegex = /```(?:json)?\s*([\s\S]*?)```/gi;
  const matches = [...cleaned.matchAll(codeFenceRegex)];
  if (matches.length > 0) {
    // Use the last code block (most likely the final answer)
    const lastMatch = matches[matches.length - 1];
    const potentialJson = lastMatch[1].trim();
    // Verify it looks like JSON (starts with { or [)
    if (potentialJson.startsWith("{") || potentialJson.startsWith("[")) {
      cleaned = potentialJson;
    }
  } else if (cleaned.startsWith("```")) {
    // Fallback for unclosed code fences
    cleaned = cleaned.replace(/^```(?:json)?\s*/i, "").replace(/```\s*$/i, "");
  }

  const lastBrace = cleaned.lastIndexOf("}");
  if (lastBrace >= 0) {
    let depth = 0;
    let start = -1;
    for (let i = lastBrace; i >= 0; i--) {
      if (cleaned[i] === "}") depth++;
      if (cleaned[i] === "{") depth--;
      if (depth === 0) {
        start = i;
        break;
      }
    }
    if (start >= 0) {
      cleaned = cleaned.slice(start, lastBrace + 1);
    }
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(cleaned);
  } catch {
    // The model may have output raw file contents instead of wrapping in
    // {"contents": "..."}. If the label looks like an implementation or analysis step
    // and the content looks like source code, auto-wrap it.
    if ((opts.label.startsWith("implementation:") || opts.label === "l3:analysis") && cleaned.length > 20) {
      dbg(
        `${opts.label}: JSON parse failed — auto-wrapping as {contents} (${cleaned.length}ch)`
      );
      parsed = { contents: cleaned, deleted: false, notes: "auto-wrapped: model returned raw code instead of JSON" };
    } else {
      throw new Error(
        `claude answer is not valid JSON (label: ${opts.label}): ` +
          `First 500 chars: ${cleaned.slice(0, 500)}`
      );
    }
  }

  return { result: parsed as T, sessionId };
}
