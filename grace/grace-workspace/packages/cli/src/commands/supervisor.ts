import fs from "node:fs";
import path from "node:path";
import url from "node:url";

import {
  SessionManager,
  SessionSupervisor,
  StateManager,
  loadConfig,
  setConfig,
} from "@10xgrace/core";
import { runDashboardOnly } from "@10xgrace/parity-core";

interface SupervisorOpts {
  config?: string;
}

/**
 * How often to refresh the parity tree in the background. Mirrors PR
 * Resolver's 300s polling cadence — keeps the GitHub sub-issue walk
 * fresh without the user having to keep the dashboard tab open or
 * click "↻ refresh".
 */
const PARITY_REFRESH_INTERVAL_MS = 5 * 60 * 1000;

/**
 * Boot the multi-session supervisor. Replaces the single-engine `run`
 * subcommand as the default for `pnpm dev`. The supervisor itself does NOT
 * run any pipeline checkpoints — it owns child engine processes (one per
 * active session) and exposes a control WebSocket on cfg.wsPort for the
 * dashboard's Homepage / sessions API.
 */
export async function supervisorCommand(opts: SupervisorOpts): Promise<void> {
  const cfg = loadConfig(opts.config);
  setConfig(cfg);

  assertProjectRoot(cfg.projectRoot);

  const state = new StateManager();
  state.ensureDefaultSession(cfg.projectRoot);

  // Wipe any orphan locks left over from a hard restart. The supervisor's
  // recoverFromCrash() also reaps PID-vanished children, but we run this
  // first so even pre-Phase-3 leftovers (no pid recorded) get cleaned.
  state.recoverStaleSessions(5_000);

  const sessions = new SessionManager(state);
  const cliEntryPath = resolveCliEntry();

  // eslint-disable-next-line no-console
  console.log(
    `\x1b[1m\x1b[35m[supervisor]\x1b[0m control ws=ws://localhost:${cfg.wsPort} cli=${cliEntryPath}`
  );

  new SessionSupervisor(state, sessions, cfg.wsPort, {
    cliEntryPath,
    configPath: opts.config,
  });

  // Parity refresh loop — keeps `.cache/tree-YYYY-MM-DD.json` warm so the
  // dashboard's /api/parity/tree.json serves fresh data even when no user is
  // poking the page. Fire-and-forget; errors are logged but don't crash the
  // supervisor (transient GitHub rate limits / network blips shouldn't take
  // the whole dev stack down).
  void startParityRefreshLoop();

  // Park forever. The supervisor's own SIGTERM/SIGINT handlers exit the process.
  await new Promise<void>(() => {
    /* never resolves */
  });
}

async function startParityRefreshLoop(): Promise<void> {
  let tickId = 0;
  const tick = async () => {
    tickId += 1;
    const id = tickId;
    const startedAt = Date.now();
    try {
      await runDashboardOnly({}, { force: true });
      // eslint-disable-next-line no-console
      console.log(
        `\x1b[90m[supervisor] parity refresh #${id} ok (${Date.now() - startedAt}ms)\x1b[0m`,
      );
    } catch (err) {
      // eslint-disable-next-line no-console
      console.warn(
        `\x1b[33m[supervisor] parity refresh #${id} failed:\x1b[0m`,
        err instanceof Error ? err.message : err,
      );
    }
  };

  // Run once on boot so the cache is warm immediately, then on a 5-min loop.
  // Initial tick is deferred via setImmediate so the supervisor's WS listen
  // isn't blocked by a slow GitHub round-trip during startup.
  setImmediate(() => void tick());
  const timer = setInterval(() => void tick(), PARITY_REFRESH_INTERVAL_MS);
  timer.unref?.();
  // eslint-disable-next-line no-console
  console.log(
    `\x1b[1m\x1b[35m[supervisor]\x1b[0m parity refresh loop enabled · interval=${PARITY_REFRESH_INTERVAL_MS / 1000}s`,
  );
}

/**
 * Resolve the absolute path to packages/cli/dist/index.js. We pass this to the
 * supervisor so it can spawn `node <cliEntry> run --session …`. Since this
 * file lives inside the same dist tree as the CLI entry, we walk up from
 * import.meta.url.
 */
function resolveCliEntry(): string {
  const here = url.fileURLToPath(import.meta.url);
  // here = .../packages/cli/dist/commands/supervisor.js
  return path.resolve(path.dirname(here), "..", "index.js");
}

function assertProjectRoot(projectRoot: string): void {
  if (!projectRoot || !projectRoot.trim()) {
    // eslint-disable-next-line no-console
    console.error(
      "\x1b[31m[supervisor] projectRoot is not configured.\x1b[0m\n" +
        "  Set TENXGRACE_PROJECT_ROOT to the absolute path of the target repo, " +
        "or set `projectRoot:` in config.yml."
    );
    process.exit(1);
  }
  if (!fs.existsSync(projectRoot)) {
    // eslint-disable-next-line no-console
    console.error(
      `\x1b[31m[supervisor] projectRoot does not exist:\x1b[0m ${projectRoot}\n` +
        "  Check TENXGRACE_PROJECT_ROOT or `projectRoot:` in config.yml."
    );
    process.exit(1);
  }
}
