import { randomBytes } from "node:crypto";
import { promises as fs } from "node:fs";
import fsSync from "node:fs";
import os from "node:os";
import path from "node:path";
import type { CheckpointId } from "./types.js";

/** Current home directory for all 10xgrace engine state (pipeline DB, session worktrees, resume handoff). */
export function tenxgraceHome(): string {
  return path.join(os.homedir(), ".tenxgrace");
}

let cachedWorkspaceRoot: string | null | undefined;

/**
 * Walk up from `startDir` looking for a pnpm workspace marker. Lets callers
 * with an arbitrary cwd (e.g. the dashboard's Vite dev server runs from
 * `packages/dashboard/`, not the workspace root) find `config.yml` without
 * passing it explicitly. Caches the first resolution so subsequent calls are
 * O(1). Returns `null` if no marker is found before hitting the filesystem
 * root — caller decides the fallback (usually `process.cwd()`).
 */
export function findWorkspaceRoot(startDir: string = process.cwd()): string | null {
  if (cachedWorkspaceRoot !== undefined) return cachedWorkspaceRoot;
  let dir = path.resolve(startDir);
  while (true) {
    if (fsSync.existsSync(path.join(dir, "pnpm-workspace.yaml"))) {
      cachedWorkspaceRoot = dir;
      return dir;
    }
    const parent = path.dirname(dir);
    if (parent === dir) break; // reached filesystem root
    dir = parent;
  }
  cachedWorkspaceRoot = null;
  return null;
}

/** Pre-rename home directory. Read once at startup so we can migrate existing state into {@link tenxgraceHome}. */
export function legacyTenxgraceHome(): string {
  return path.join(os.homedir(), ".10xgrace");
}

/**
 * One-shot, idempotent migration from `~/.10xgrace/` to `~/.tenxgrace/`. Safe to call repeatedly:
 * skips silently when there's nothing to move or when the destination already exists. Called from
 * the {@link StateManager} constructor so any subsequent home-dir reads (pipeline DB, sessions/,
 * resume.json) land at the new location. Uses fs.renameSync — both paths are immediate children
 * of $HOME so cross-device renames aren't expected; if one does fail we log and leave the legacy
 * dir in place rather than risk a half-completed copy of an open SQLite file.
 */
export function migrateLegacyTenxgraceHome(): void {
  const legacy = legacyTenxgraceHome();
  const current = tenxgraceHome();
  if (!fsSync.existsSync(legacy)) return;
  fsSync.mkdirSync(current, { recursive: true });

  const items = [
    "pipeline.sqlite",
    "pipeline.sqlite-wal",
    "pipeline.sqlite-shm",
    "resume.json",
    "sessions",
  ];
  let moved = 0;
  for (const name of items) {
    const src = path.join(legacy, name);
    const dst = path.join(current, name);
    if (!fsSync.existsSync(src)) continue;
    if (fsSync.existsSync(dst)) continue;
    try {
      fsSync.renameSync(src, dst);
      moved++;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      // eslint-disable-next-line no-console
      console.warn(`[migration] could not move ${src} -> ${dst}: ${msg}`);
    }
  }
  if (moved > 0) {
    // eslint-disable-next-line no-console
    console.log(`[migration] moved ${moved} item(s) from ~/.10xgrace -> ~/.tenxgrace`);
  }
}

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
