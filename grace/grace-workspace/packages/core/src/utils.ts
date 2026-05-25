import { randomBytes } from "node:crypto";
import { promises as fs } from "node:fs";
import path from "node:path";
import type { CheckpointId } from "./types.js";

export function newRunId(): string {
  const ts = new Date().toISOString().replace(/[:.]/g, "-");
  return `run-${ts}-${randomBytes(3).toString("hex")}`;
}

export async function withTimeout<T>(
  p: Promise<T>,
  ms: number,
  label: CheckpointId | string
): Promise<T> {
  let timer: NodeJS.Timeout | undefined;
  const timeout = new Promise<T>((_, reject) => {
    timer = setTimeout(
      () => reject(new Error(`[${label}] timed out after ${ms}ms`)),
      ms
    );
  });
  try {
    return await Promise.race([p, timeout]);
  } finally {
    if (timer) clearTimeout(timer);
  }
}

export async function atomicWrite(
  filePath: string,
  content: string
): Promise<void> {
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  const tmp = `${filePath}.${randomBytes(4).toString("hex")}.tmp`;
  await fs.writeFile(tmp, content, "utf-8");
  await fs.rename(tmp, filePath);
}

export async function ensureDir(dir: string): Promise<void> {
  await fs.mkdir(dir, { recursive: true });
}

export function artifactsDir(projectRoot: string): string {
  return path.join(projectRoot, "pipeline-artifacts");
}

export function stripJsonFences(text: string): string {
  let t = text.trim();
  // 1. Whole body wrapped in a ```json ... ``` fence → pull the body.
  const fence = t.match(/^```(?:json)?\s*([\s\S]*?)\s*```$/);
  if (fence && fence[1]) t = fence[1].trim();
  // 2. Inline ```json ... ``` block somewhere inside prose → try that first.
  const inlineFence = t.match(/```(?:json)?\s*([\s\S]*?)\s*```/);
  if (inlineFence && inlineFence[1]) {
    const inner = inlineFence[1].trim();
    try {
      JSON.parse(inner);
      return inner;
    } catch {
      /* fall through */
    }
  }
  // 3. Already valid → return as-is.
  try {
    JSON.parse(t);
    return t;
  } catch {
    /* fall through */
  }
  // 4. Scan for the first balanced {...} or [...] block and return it.
  const extracted = extractBalancedJson(t);
  return extracted ?? t;
}

function extractBalancedJson(text: string): string | null {
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (ch !== "{" && ch !== "[") continue;
    const open = ch;
    const close = ch === "{" ? "}" : "]";
    let depth = 0;
    let inStr = false;
    let escape = false;
    for (let j = i; j < text.length; j++) {
      const c = text[j]!;
      if (escape) {
        escape = false;
        continue;
      }
      if (inStr) {
        if (c === "\\") escape = true;
        else if (c === '"') inStr = false;
        continue;
      }
      if (c === '"') {
        inStr = true;
        continue;
      }
      if (c === open) depth++;
      else if (c === close) {
        depth--;
        if (depth === 0) {
          const slice = text.slice(i, j + 1);
          try {
            JSON.parse(slice);
            return slice;
          } catch {
            break;
          }
        }
      }
    }
  }
  return null;
}

export function safeParseJson<T = unknown>(text: string): T | null {
  try {
    return JSON.parse(stripJsonFences(text)) as T;
  } catch {
    return null;
  }
}

export function nowIso(): string {
  return new Date().toISOString();
}
