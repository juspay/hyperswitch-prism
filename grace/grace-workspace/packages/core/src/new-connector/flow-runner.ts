import fs from "node:fs";
import path from "node:path";
import { runAI } from "../tools/runner-factory.js";

/**
 * Per-flow subagent invocation, as `.gracerules` PHASE 3 prescribes.
 *
 * Each flow gets its own Claude session with its own session-id (so
 * resume-from-flow works without cross-contamination), its own prompt
 * cache, and its own visible checkpoint row in the dashboard. The
 * subagent prompt is built from `.gracerules` directly — we read it at
 * call time and include both the shared FLOW SUBAGENT RESPONSIBILITIES
 * template (lines 244–477) and the flow-specific metadata block (lines
 * 481–570).
 *
 * Live stdout is forwarded to `onStdoutLine` so the dashboard's centre
 * spinner shows each tool use (📖 Read /foo, ✏️ Edit /bar, ▶ Bash cargo
 * check) as Claude executes — instead of just a rotating spinner.
 */

export interface RunFlowInput {
  /** Connector name in PascalCase (e.g. "Stripe"). */
  connector: string;
  /** Flow name in canonical `.gracerules` casing (e.g. "Authorize"). */
  flow: string;
  /** Absolute path to the project root (the hyperswitch-prism repo). */
  projectRoot: string;
  /** Absolute path to the techspec markdown for this connector. */
  specPath: string;
  /** Absolute path to `.gracerules`. */
  gracerulesPath: string;
  /**
   * Per-line stdout callback. Checkpoints wire this to `ctx.log` so the
   * dashboard's centre spinner streams agent progress live.
   */
  onStdoutLine?: (line: string) => void;
  /** Optional Claude session id from a prior attempt (for resume). */
  priorSessionId?: string;
  /** Optional Claude model override. */
  claudeModel?: string;
}

export interface RunFlowResult {
  status: "passed" | "failed";
  attempts: number;
  durationMs: number;
  claudeSessionId?: string;
  lastError?: string;
  /** Last ~200 chars of Claude's final answer, for log surfacing. */
  summary?: string;
}

const PER_FLOW_TIMEOUT_MS = 30 * 60 * 1000; // 30 min per flow

/**
 * Run one flow's implementation. Single attempt — the orchestrator's
 * checkpoint retry mechanism handles re-runs (the engine's `maxRetries`
 * is the right place to gate that, not an inner loop).
 *
 * Throws are caught and converted to `status: "failed"` with the error
 * message in `lastError`, so the checkpoint can decide what to do.
 */
export async function runFlowImplementation(
  input: RunFlowInput
): Promise<RunFlowResult> {
  const startedAt = Date.now();

  let gracerules = "";
  try {
    gracerules = fs.readFileSync(input.gracerulesPath, "utf-8");
  } catch (err) {
    return {
      status: "failed",
      attempts: 0,
      durationMs: Date.now() - startedAt,
      lastError: `Could not read .gracerules at ${input.gracerulesPath}: ${
        err instanceof Error ? err.message : String(err)
      }`,
    };
  }

  const flowBlock = extractFlowSubagentBlock(gracerules, input.flow);
  const sharedTemplate = extractSharedTemplate(gracerules);

  const systemPrompt = [
    `You are the **${input.flow} Flow Implementation Subagent** for the`,
    `**${input.connector}** payment connector in the hyperswitch-prism`,
    `(UCS) Rust codebase.`,
    ``,
    `You are operating under \`.gracerules\` PHASE 3 — one subagent per flow,`,
    `executed sequentially. Your job is to implement EXACTLY the ${input.flow}`,
    `flow. Do not touch other flows; another subagent owns each of them.`,
    ``,
    `# Inputs`,
    ``,
    `- **Connector:** ${input.connector}`,
    `- **Flow:** ${input.flow}`,
    `- **Project root:** ${input.projectRoot}`,
    `- **Techspec:** ${input.specPath}`,
    `- **.gracerules:** ${input.gracerulesPath}`,
    ``,
    `# Flow-specific metadata from .gracerules`,
    ``,
    flowBlock || `(no per-flow block found for "${input.flow}" — fall back to .gracerules PHASE 3 generic guidance.)`,
    ``,
    `# Shared FLOW SUBAGENT RESPONSIBILITIES template from .gracerules`,
    ``,
    sharedTemplate,
    ``,
    `# Work to do`,
    ``,
    `Follow the MANDATORY WORKFLOW (STEP 1 through STEP 7) from the shared`,
    `template above, scoped to this single flow only. Read the techspec, read`,
    `the pattern file referenced in the flow block, read available utils and`,
    `enums, write the flow's macro registration and transformer code, run`,
    `\`cargo build --package connector-integration\`, fix any compilation`,
    `errors using UCS conventions, and report when done.`,
    ``,
    `When you finish, respond with a single line:`,
    `\`DONE — ${input.flow} flow implemented for ${input.connector}\``,
    `followed by a one-paragraph summary of the changes you made.`,
  ].join("\n");

  try {
    const { result, sessionId } = await runAI<string>({
      skillBody: input.priorSessionId ? "" : systemPrompt,
      userPayload: input.priorSessionId
        ? `Continue: implement the ${input.flow} flow per the prior plan. Address any cargo build errors that surfaced. Reply with the same DONE line when complete.`
        : `Begin implementation of the ${input.flow} flow for ${input.connector}.`,
      cwd: input.projectRoot,
      label: `new-connector-flow-${input.connector}-${input.flow}`,
      sessionLabel: `new-connector-${input.connector}-${input.flow}`,
      timeoutMs: PER_FLOW_TIMEOUT_MS,
      rawText: true,
      allowWrite: true,
      claudeSessionId: input.priorSessionId,
      incremental: !!input.priorSessionId,
      model: input.claudeModel,
      onStdoutLine: input.onStdoutLine,
    });

    const summary =
      typeof result === "string"
        ? result.slice(-200).trim()
        : JSON.stringify(result).slice(-200);
    const passed =
      typeof result === "string" &&
      result.toLowerCase().includes(`done — ${input.flow.toLowerCase()} flow`);

    return {
      status: passed ? "passed" : "failed",
      attempts: 1,
      durationMs: Date.now() - startedAt,
      claudeSessionId: sessionId,
      summary,
      lastError: passed ? undefined : "Subagent did not return a DONE marker",
    };
  } catch (err) {
    return {
      status: "failed",
      attempts: 1,
      durationMs: Date.now() - startedAt,
      lastError: err instanceof Error ? err.message : String(err),
    };
  }
}

/**
 * Pluck the per-flow metadata block from `.gracerules`. Returns the lines
 * from the `#### <FLOW> FLOW SUBAGENT` heading up to the next `####`
 * heading (or EOF). Empty string when not found — caller falls back to the
 * shared template.
 */
function extractFlowSubagentBlock(gracerules: string, flow: string): string {
  const upper = flow.toUpperCase();
  const lines = gracerules.split("\n");
  let start = -1;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] ?? "";
    if (line.startsWith("####") && line.toUpperCase().includes(`${upper} FLOW SUBAGENT`)) {
      start = i;
      break;
    }
  }
  if (start === -1) return "";
  let end = lines.length;
  for (let i = start + 1; i < lines.length; i++) {
    if ((lines[i] ?? "").startsWith("####")) {
      end = i;
      break;
    }
  }
  return lines.slice(start, end).join("\n").trim();
}

/**
 * Pluck the shared "FLOW SUBAGENT RESPONSIBILITIES" template from
 * `.gracerules`. Starts at "## FLOW SUBAGENT RESPONSIBILITIES" and stops
 * at the next `## ` heading. Empty string when not found.
 */
function extractSharedTemplate(gracerules: string): string {
  const lines = gracerules.split("\n");
  let start = -1;
  for (let i = 0; i < lines.length; i++) {
    if ((lines[i] ?? "").startsWith("## FLOW SUBAGENT RESPONSIBILITIES")) {
      start = i;
      break;
    }
  }
  if (start === -1) return "";
  let end = lines.length;
  for (let i = start + 1; i < lines.length; i++) {
    const line = lines[i] ?? "";
    if (line.startsWith("## ") && !line.startsWith("### ")) {
      end = i;
      break;
    }
  }
  return lines.slice(start, end).join("\n").trim();
}

/** Default `.gracerules` location relative to project root. */
export function defaultGracerulesPath(projectRoot: string): string {
  return path.join(projectRoot, "grace", "rulesbook", "codegen", ".gracerules");
}

/** Default techspec location for a connector. */
export function defaultSpecPath(projectRoot: string, connector: string): string {
  return path.join(
    projectRoot,
    "grace",
    "rulesbook",
    "codegen",
    "references",
    connector,
    "technical_specification.md"
  );
}
