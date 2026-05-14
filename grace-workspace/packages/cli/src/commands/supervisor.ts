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

interface SupervisorOpts {
  config?: string;
}

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

  // Park forever. The supervisor's own SIGTERM/SIGINT handlers exit the process.
  await new Promise<void>(() => {
    /* never resolves */
  });
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
