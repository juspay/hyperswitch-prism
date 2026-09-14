import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import { StateManager } from "@10xgrace/core";

/**
 * Phase 12: prune stale Claude session jsonl files from
 * `~/.claude/projects/<encoded-cwd>/<uuid>.jsonl`.
 *
 * Grace creates one such file per LLM-using phase per pipeline run (4-6 per
 * run, ~50KB-1MB each). Without periodic cleanup these accumulate forever —
 * ~5 GB/year at 10 runs/day. This command reaps anything older than the cutoff
 * that isn't referenced by an active run.
 *
 * Active runs are determined by walking SQLite's `runs` table for rows whose
 * `status` is `pending` or `running`, then extracting any UUID-shaped string
 * values from each run's `artifacts_json`. Those uuids are spared regardless
 * of file age. Everything else past the cutoff is deleted.
 *
 * Safe to run anytime — if a run is currently using a session id, that id is
 * in `runs.artifacts_json` and gets the active-set skip.
 */

export interface SessionsPruneOpts {
  olderThan?: string; // "30d", "7d", "12h", "0d" (everything not active)
  dryRun?: boolean;
}

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function parseDuration(spec: string): number {
  const match = /^(\d+)\s*(d|h|m|s)$/i.exec(spec.trim());
  if (!match) {
    throw new Error(
      `Invalid --older-than value: "${spec}". Use forms like "30d", "12h", "0d".`
    );
  }
  const n = parseInt(match[1]!, 10);
  switch (match[2]!.toLowerCase()) {
    case "d":
      return n * 24 * 60 * 60 * 1000;
    case "h":
      return n * 60 * 60 * 1000;
    case "m":
      return n * 60 * 1000;
    case "s":
      return n * 1000;
    default:
      throw new Error(`Unreachable: ${match[2]}`);
  }
}

/**
 * Walk every JSON value in `node` recursively and collect any string leaves
 * that look like UUIDs. Cheap and avoids having to know about each phase's
 * specific session-id field name (l2LinksSessionId, l3SessionId, etc.).
 */
function collectUuids(node: unknown, into: Set<string>): void {
  if (!node) return;
  if (typeof node === "string") {
    if (UUID_RE.test(node)) into.add(node);
    return;
  }
  if (Array.isArray(node)) {
    for (const item of node) collectUuids(item, into);
    return;
  }
  if (typeof node === "object") {
    for (const v of Object.values(node)) collectUuids(v, into);
  }
}

async function gatherActiveSessionIds(state: StateManager): Promise<Set<string>> {
  const active = new Set<string>();
  // Pull artifacts_json from runs that haven't terminated yet, plus any
  // session that's currently locked (current_run_id is set).
  const runs = await state.listRuns();
  for (const summary of runs) {
    if (summary.status === "running" || summary.status === "pending") {
      try {
        const loaded = await state.load(summary.runId);
        if (loaded?.artifacts) {
          collectUuids(loaded.artifacts, active);
        }
      } catch {
        // If load fails for one run, don't block pruning of unrelated ids.
      }
    }
  }
  // Also include any run referenced by a session's current_run_id, even if
  // its status row hasn't been updated yet — protects mid-checkpoint state.
  for (const s of state.listSessions()) {
    if (s.currentRunId) {
      try {
        const loaded = await state.load(s.currentRunId);
        if (loaded?.artifacts) collectUuids(loaded.artifacts, active);
      } catch {
        /* see above */
      }
    }
  }
  return active;
}

export async function sessionsPruneCommand(
  opts: SessionsPruneOpts
): Promise<void> {
  const cutoffMs = parseDuration(opts.olderThan ?? "30d");
  const cutoffTime = Date.now() - cutoffMs;
  const projectsDir = path.join(os.homedir(), ".claude", "projects");

  let projectDirs: string[];
  try {
    projectDirs = await fs.readdir(projectsDir);
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") {
      // eslint-disable-next-line no-console
      console.log(
        `No ~/.claude/projects directory found — nothing to prune.`
      );
      return;
    }
    throw err;
  }

  const state = new StateManager();
  const active = await gatherActiveSessionIds(state);

  let deleted = 0;
  let skippedActive = 0;
  let skippedFresh = 0;
  let bytesFreed = 0;

  for (const projDir of projectDirs) {
    const projPath = path.join(projectsDir, projDir);
    let entries: string[];
    try {
      entries = await fs.readdir(projPath);
    } catch {
      continue;
    }
    for (const entry of entries) {
      if (!entry.endsWith(".jsonl")) continue;
      const uuid = entry.slice(0, -6);
      if (!UUID_RE.test(uuid)) continue;
      if (active.has(uuid)) {
        skippedActive++;
        continue;
      }
      const filePath = path.join(projPath, entry);
      let stat;
      try {
        stat = await fs.stat(filePath);
      } catch {
        continue;
      }
      if (stat.mtimeMs >= cutoffTime) {
        skippedFresh++;
        continue;
      }
      bytesFreed += stat.size;
      if (opts.dryRun) {
        // eslint-disable-next-line no-console
        console.log(`(dry-run) would delete ${filePath} (${stat.size} bytes)`);
      } else {
        try {
          await fs.unlink(filePath);
        } catch (err) {
          // eslint-disable-next-line no-console
          console.error(
            `Failed to delete ${filePath}: ${err instanceof Error ? err.message : String(err)}`
          );
          continue;
        }
      }
      deleted++;
    }
  }

  const mb = (bytesFreed / 1024 / 1024).toFixed(2);
  const verb = opts.dryRun ? "would delete" : "deleted";
  // eslint-disable-next-line no-console
  console.log(
    `10xgrace sessions prune: ${verb} ${deleted} file(s), freed ${mb} MB. ` +
      `Skipped ${skippedActive} active, ${skippedFresh} newer than cutoff (${opts.olderThan ?? "30d"}).`
  );
}
