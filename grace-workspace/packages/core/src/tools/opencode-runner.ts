import { spawn } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";
import http from "node:http";
import { getConfig } from "../config.js";

/**
 * Shells out to the local `opencode` CLI to run a planning step.
 *
 * Why this exists: our previous approach of calling the LLM directly with a
 * tool loop relied on the model voluntarily calling read_file/grep/spawn_agent
 * before producing JSON. Weaker models (kimi) skip those tool calls entirely,
 * hallucinate file paths, and emit "create" for files that should be modified.
 *
 * opencode's own runtime forces the model to actually use tools and reliably
 * loads SKILL.md files. So for L3/L4 planning steps we delegate the whole
 * reasoning loop to opencode and just capture its final JSON answer.
 *
 * To avoid brittle stdout parsing we instruct the prompt to write the final
 * JSON to a known temp file (using opencode's built-in write tool), then
 * read that file after opencode exits. opencode's stdout is only used for
 * progress logging.
 */

export interface OpencodeRunOptions {
  /** The skill body / system prompt — describes what to produce. */
  skillBody: string;
  /** Structured user payload (JSON-serializable). */
  userPayload: unknown;
  /** Working directory opencode should run in (the target repo root). */
  cwd: string;
  /** Short label for logs. */
  label: string;
  /** Optional model override in opencode "provider/model" format. */
  model?: string;
  /** Hard timeout in ms. Default 10 min. */
  timeoutMs?: number;
  /**
   * If true, the answer file is treated as raw text and returned as a string
   * instead of JSON.parse'd. Use for research / summary agents that return
   * markdown or plain prose.
   */
  rawText?: boolean;
}

function dbg(...args: unknown[]) {
  // eslint-disable-next-line no-console
  console.error("\x1b[90m[opencode]", ...args, "\x1b[0m");
}

export interface OpencodeHealthResult {
  connected: boolean;
  url: string;
  error?: string;
  latencyMs?: number;
}

/**
 * Ping the opencode server to check if it's reachable. Performs a simple HTTP
 * GET to the configured attachUrl. Returns a result object with connectivity
 * status, latency, and any error details.
 */
export async function checkOpencodeHealth(): Promise<OpencodeHealthResult> {
  const oc = getConfig().opencode;
  const url = process.env.TENXGRACE_OPENCODE_ATTACH ?? oc.attachUrl ?? "";

  if (!url) {
    return { connected: false, url: "(not configured)", error: "No attachUrl configured — opencode will spawn fresh processes" };
  }

  const start = Date.now();
  return new Promise<OpencodeHealthResult>((resolve) => {
    const req = http.get(url, { timeout: 5000 }, (res) => {
      // Any response (even 404) means the server is listening.
      res.resume(); // drain the response
      resolve({ connected: true, url, latencyMs: Date.now() - start });
    });
    req.on("error", (err) => {
      resolve({ connected: false, url, error: err.message, latencyMs: Date.now() - start });
    });
    req.on("timeout", () => {
      req.destroy();
      resolve({ connected: false, url, error: "Connection timed out (5s)", latencyMs: Date.now() - start });
    });
  });
}

export async function runOpencode<T = unknown>(
  opts: OpencodeRunOptions
): Promise<T> {
  const absCwdEarly = path.resolve(opts.cwd);

  const answerInstructions = opts.rawText
    ? [
        "## How to return your answer",
        "",
        "IMPORTANT: Do NOT use your write or edit tools to create any files. Do NOT create answer.json or any other output files. Just reply in chat with your final report as plain text (markdown is fine).",
        "",
        "Use your read tools (read_file, list_dir, glob, grep, webfetch, websearch) to gather evidence BEFORE replying. Then reply with the report directly in your message.",
      ]
    : [
        "## How to return your answer",
        "",
        "IMPORTANT: Do NOT use your write or edit tools to create any files. Do NOT create answer.json or any other output files. Do NOT write prose or explanation — reply with ONLY the raw JSON object in your chat message. No markdown fences, no thinking out loud, no preamble. The very first character of your reply must be `{` and the very last must be `}`.",
        "",
        "Use your read tools (read_file, list_dir, glob, grep) to gather evidence BEFORE replying. Then reply with ONLY the JSON.",
      ];

  const prompt = [
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

  const absCwd = absCwdEarly;

  const oc = getConfig().opencode;
  // Caller override > config.yml > default. Env vars are escape hatches only.
  const attachUrl =
    process.env.TENXGRACE_OPENCODE_ATTACH ?? oc.attachUrl ?? "";
  const model =
    opts.model ?? process.env.TENXGRACE_OPENCODE_MODEL ?? oc.model;

  const args = ["run"];
  if (attachUrl) {
    args.push("--attach", attachUrl);
  }
  if (model) {
    args.push("--model", model);
  }
  args.push(
    "--dir",
    absCwd,
    "--print-logs",
    "--log-level",
    "INFO",
    prompt
  );

  const timeoutMs = opts.timeoutMs ?? oc.timeoutMs ?? 10 * 60 * 1000;
  const startedAt = Date.now();
  // eslint-disable-next-line no-console
  console.log(
    `\x1b[36m[opencode] → ${opts.label} · attach=${attachUrl} · cwd=${absCwd} · model=${model} · prompt=${prompt.length}ch\x1b[0m`
  );

  // Capture stdout and stderr to extract the model's answer and error details.
  const stdoutChunks: Buffer[] = [];
  const stderrChunks: Buffer[] = [];

  await new Promise<void>((resolve, reject) => {
    const child = spawn("opencode", args, {
      cwd: absCwd,
      stdio: ["ignore", "pipe", "pipe"],
      env: process.env,
    });

    // Capture stdout into a buffer AND mirror it to process.stdout so the
    // user still sees opencode's progress output in the engine logs.
    child.stdout!.on("data", (chunk: Buffer) => {
      stdoutChunks.push(chunk);
      process.stdout.write(chunk);
    });

    // Capture stderr for error diagnostics AND mirror to process.stderr.
    child.stderr!.on("data", (chunk: Buffer) => {
      stderrChunks.push(chunk);
      process.stderr.write(chunk);
    });

    const timer = setTimeout(() => {
      dbg(`timeout after ${timeoutMs}ms — killing opencode`);
      child.kill("SIGTERM");
      setTimeout(() => child.kill("SIGKILL"), 5000).unref();
      const stderr = Buffer.concat(stderrChunks).toString("utf-8").trim();
      const detail = stderr ? ` — stderr: ${stderr.slice(-500)}` : "";
      reject(new Error(`opencode timed out after ${timeoutMs}ms (label: ${opts.label})${detail}`));
    }, timeoutMs);
    timer.unref();

    child.on("error", (err) => {
      clearTimeout(timer);
      reject(new Error(`opencode spawn failed: ${err.message}`));
    });
    child.on("exit", (code, signal) => {
      clearTimeout(timer);
      const stderr = Buffer.concat(stderrChunks).toString("utf-8").trim();
      const detail = stderr ? ` — stderr: ${stderr.slice(-500)}` : "";
      if (signal) {
        reject(new Error(`opencode killed by signal ${signal} (label: ${opts.label})${detail}`));
      } else if (code !== 0) {
        reject(new Error(`opencode exited with code ${code} (label: ${opts.label})${detail}`));
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
  // Also clean up the .10xgrace-opencode-tmp directory if it exists
  fs.rm(path.join(absCwd, ".10xgrace-opencode-tmp"), { recursive: true, force: true }).catch(() => undefined);

  const elapsed = Date.now() - startedAt;
  const raw = Buffer.concat(stdoutChunks).toString("utf-8");
  // eslint-disable-next-line no-console
  console.log(`\x1b[36m[opencode] ← ${opts.label} · ${elapsed}ms · stdout=${raw.length}ch\x1b[0m`);

  if (!raw.trim()) {
    throw new Error(
      `opencode finished but produced no output (label: ${opts.label}). stdout was empty.`
    );
  }

  // Strip BOM and ANSI escape codes.
  // eslint-disable-next-line no-control-regex
  let cleaned = raw.trim().replace(/^\uFEFF/, "").replace(/\x1b\[[0-9;]*m/g, "");

  if (opts.rawText) {
    // Drop opencode chrome lines ("> build · open-large", "[0m", etc.)
    const lines = cleaned.split("\n");
    const contentStart = lines.findIndex(
      (l) => l.trim().length > 0 && !l.trim().startsWith(">") && !l.startsWith("[0m")
    );
    if (contentStart >= 0) {
      cleaned = lines.slice(contentStart).join("\n").trim();
    }
    return cleaned as unknown as T;
  }

  // JSON mode: find the last complete JSON object in the output.
  // The model's chat reply may include tool-call chatter before the final JSON.
  if (cleaned.startsWith("```")) {
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
    // {"contents": "..."}. If the label looks like an implementation step
    // and the content looks like source code, auto-wrap it.
    if (opts.label.startsWith("implementation:") && cleaned.length > 20) {
      dbg(
        `${opts.label}: JSON parse failed — auto-wrapping as {contents} (${cleaned.length}ch)`
      );
      parsed = { contents: cleaned, deleted: false, notes: "auto-wrapped: model returned raw code instead of JSON" };
    } else {
      throw new Error(
        `opencode answer is not valid JSON (label: ${opts.label}): ` +
          `First 500 chars: ${cleaned.slice(0, 500)}`
      );
    }
  }

  return parsed as T;
}
