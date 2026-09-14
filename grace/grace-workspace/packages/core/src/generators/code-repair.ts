import { promises as fs } from "node:fs";
import path from "node:path";
import { runAI } from "../tools/runner-factory.js";
import { atomicWrite } from "../utils.js";
import type { PipelineContext } from "../types.js";

const REPAIR_SYSTEM = `You are a senior ReScript/React/TypeScript engineer fixing build, type, or test errors in the hyperswitch-control-center project.

You will receive:
1. A list of affected files with their current source code
2. The error messages produced by the compiler/tests
3. The L4 spec to ensure fixes align with the planned implementation

Use your tools (read_file, glob, grep) to understand the codebase context before fixing. Check related files, imports, and patterns to ensure the fix is consistent with existing code.

For multiple files, SPAWN MULTIPLE SUB-AGENTS in parallel to fix different files simultaneously. Each sub-agent should focus on one file or a small group of related files.

Return ONLY a JSON object:
{
  "fixes": [
    {
      "filePath": "path/to/file.res",
      "content": "the complete corrected file content"
    }
  ]
}

No explanations, no markdown fences, no preamble. Just the raw JSON with corrected file contents that compile cleanly.`;

export function extractAffectedFiles(
  errors: string[],
  projectRoot: string
): string[] {
  const out = new Set<string>();
  const rePath = /([A-Za-z0-9_\-./]+\.(?:res|resi|ts|tsx|js|jsx))/g;
  for (const err of errors) {
    const matches = err.matchAll(rePath);
    for (const m of matches) {
      const p = m[1]!;
      const abs = path.isAbsolute(p) ? p : path.join(projectRoot, p);
      out.add(abs);
    }
  }
  return [...out];
}

interface FileContentInfo {
  path: string;
  content: string;
  errors: string[];
}

interface FixResult {
  filePath: string;
  content: string;
}

interface RepairsResult {
  fixes: FixResult[];
}

export async function repairCode(
  ctx: PipelineContext,
  errors: string[]
): Promise<void> {
  const files = extractAffectedFiles(errors, ctx.task.projectRoot);
  if (files.length === 0) {
    ctx.log("[code-repair] No files extracted from errors; skipping.", "warn");
    return;
  }

  // Read all affected files upfront
  const fileContents: FileContentInfo[] = [];
  for (const filePath of files) {
    let existing: string;
    try {
      existing = await fs.readFile(filePath, "utf-8");
    } catch {
      ctx.log("[code-repair] Cannot read " + filePath + "; skipping.", "warn");
      continue;
    }
    const relevantErrors = errors.filter((e) =>
      e.includes(path.basename(filePath))
    );
    fileContents.push({
      path: filePath,
      content: existing,
      errors: relevantErrors.length ? relevantErrors : errors,
    });
  }

  if (fileContents.length === 0) {
    ctx.log("[code-repair] No readable files to repair; skipping.", "warn");
    return;
  }

  ctx.log("[code-repair] Requesting fixes for " + fileContents.length + " file(s) via AI runner", "info");

  let repairs: RepairsResult;
  try {
    ({ result: repairs } = await runAI<RepairsResult>({
      skillBody: REPAIR_SYSTEM,
      userPayload: {
        files: fileContents,
        l3Analysis: ctx.artifacts.l3,
      },
      cwd: ctx.task.projectRoot,
      label: "code-repair",
      timeoutMs: 15 * 60 * 1000,
    }));
  } catch (err) {
    ctx.log(
      "[code-repair] AI runner call failed: " + (err instanceof Error ? err.message : String(err)),
      "error"
    );
    return;
  }

  if (!repairs?.fixes || !Array.isArray(repairs.fixes)) {
    ctx.log("[code-repair] Invalid response from AI runner - expected { fixes: [...] }", "error");
    return;
  }

  for (const fix of repairs.fixes) {
    if (!fix.filePath || !fix.content) {
      ctx.log("[code-repair] Skipping invalid fix entry", "warn");
      continue;
    }
    if (!fix.content.trim()) {
      ctx.log("[code-repair] Empty content for " + fix.filePath + "; skipping.", "warn");
      continue;
    }
    await atomicWrite(fix.filePath, fix.content);
    ctx.log("[code-repair] Rewrote: " + fix.filePath, "success");
  }
}
