import { Command } from "commander";
import { loadParityConfig } from "./config.js";
import { runDashboardOnly, runHeartbeat, runLoop, runSweepOnly } from "./orchestrator.js";
import { getIssue } from "./github/client.js";
import { getLeafRow } from "./persistence.js";
import { StateManager } from "@10xgrace/core";
import { extendForParity } from "./persistence.js";

function parseInterval(s: string): number {
  const m = s.match(/^(\d+)(ms|s|m|h)$/);
  if (!m) throw new Error(`bad interval: ${s} (use 30s, 5m, 1h)`);
  const n = Number(m[1]);
  const u = m[2];
  if (u === "ms") return n;
  if (u === "s") return n * 1000;
  if (u === "m") return n * 60 * 1000;
  if (u === "h") return n * 60 * 60 * 1000;
  return n;
}

export function registerParityCommands(program: Command): void {
  const parity = program.command("parity").description("Parity Autopilot — bring prism into response parity with hyperswitch");

  parity
    .command("tick")
    .description("Run one heartbeat (one leaf max) and exit")
    .option("--leaf <n>", "Run autopilot for a specific leaf number (overrides FIFO pick)", (v) => parseInt(v, 10))
    .option("--dry-run", "Skip claim + handoff: no GitHub label, no comment, no PR. Agents + cargo + grpc verify still run.")
    .option(
      "--runner <name>",
      "Override LLM runner for this heartbeat (claude-code | opencode). Defaults to parityAutopilot.llm.runner from config.yml.",
      (v: string) => {
        if (v !== "claude-code" && v !== "opencode") {
          throw new Error(`--runner must be claude-code or opencode, got: ${v}`);
        }
        return v as "claude-code" | "opencode";
      },
    )
    .option(
      "--bridge-path <path>",
      "Override PARITY_BRIDGE_PATH for this heartbeat (hyperswitch clone with the UCS bridge crate).",
    )
    .option(
      "--oracle-path <path>",
      "Override PARITY_ORACLE_PATH for this heartbeat (read-only hyperswitch clone for oracle reference).",
    )
    .action(async (opts: {
      leaf?: number;
      dryRun?: boolean;
      runner?: "claude-code" | "opencode";
      bridgePath?: string;
      oraclePath?: string;
    }) => {
      const r = await runHeartbeat({
        leafNumber: opts.leaf,
        dryRun: opts.dryRun,
        runnerOverride: opts.runner,
        bridgePathOverride: opts.bridgePath,
        oraclePathOverride: opts.oraclePath,
      });
      // eslint-disable-next-line no-console
      console.log(JSON.stringify(r, null, 2));
      process.exit(r.outcome === "error" ? 1 : 0);
    });

  parity
    .command("loop")
    .description("Run heartbeats forever; sleep between them")
    .option("--interval <duration>", "Sleep between heartbeats (e.g. 5m, 30s)", "5m")
    .action(async (opts: { interval: string }) => {
      await runLoop(parseInterval(opts.interval));
    });

  parity
    .command("dashboard")
    .description("Regenerate parity-dashboard.md and connectors/*.md only (no claim, no edits)")
    .action(async () => {
      await runDashboardOnly();
      // eslint-disable-next-line no-console
      console.log("dashboard refreshed");
    });

  parity
    .command("sweep")
    .description("Run sweep phase only (PR polling + label backfill + reminders)")
    .action(async () => {
      await runSweepOnly();
      // eslint-disable-next-line no-console
      console.log("sweep complete");
    });

  parity
    .command("status <leaf>")
    .description("Print all known state for one leaf (DB row + live GitHub fetch)")
    .action(async (leafArg: string) => {
      const cfg = loadParityConfig();
      const state = new StateManager();
      extendForParity(state);
      const num = Number(leafArg.replace(/^#/, ""));
      const row = getLeafRow(state, num);
      const live = await getIssue(`${cfg.github.owner}/${cfg.github.repo}`, num).catch((e) => ({ error: (e as Error).message }));
      // eslint-disable-next-line no-console
      console.log(JSON.stringify({ db: row, github: live }, null, 2));
    });
}

export type { ParityConfig } from "./config.js";
