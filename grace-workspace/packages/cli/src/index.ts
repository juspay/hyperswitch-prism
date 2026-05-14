#!/usr/bin/env node
import { Command } from "commander";
import { runCommand } from "./commands/run.js";
import { statusCommand } from "./commands/status.js";
import { replayCommand } from "./commands/replay.js";
import { historyCommand } from "./commands/history.js";
import { testLlmCommand } from "./commands/test-llm.js";
import { registerParityCommands } from "@10xgrace/parity-core";

const program = new Command();
program
  .name("10xgrace")
  .description("10XGRACE · checkpoint-based spec-driven development pipeline")
  .version("0.1.0");

program
  .command("run")
  .description("Run the pipeline end-to-end (single-session engine)")
  .option("--task-file <path>", "Load task definition from a JSON file")
  .option("--start-from <id>", "Start from a specific checkpoint")
  .option("--project <path>", "Project root path (default: from config.yml)")
  .option("--dry-run", "Simulate steps without writing files or running tests")
  .option("--max-retries <n>", "Override max retries per checkpoint", (v) => parseInt(v, 10))
  .option("--threshold <n>", "Design match threshold 0–1", (v) => parseFloat(v))
  .option("--no-dashboard", "Disable the web dashboard")
  .option("--auto-approve-reviews", "Skip human review gates — auto-approve all")
  .option("--task-from-ui", "Wait for task definition from the dashboard UI over WebSocket")
  .option("--auto-mode", "Stand-in LLM reviewer answers all clarifying questions and approves all gates on your behalf")
  .option("--review-timeout <ms>", "Timeout for human review steps", (v) => parseInt(v, 10))
  .option("--resume <runId>", "Resume a previous run from its last checkpoint")
  .option("--config <path>", "Path to config.yml")
  .option(
    "--session <id>",
    "Owning session id. Overrides task.sessionId. Used by the supervisor when spawning a child engine. Defaults to 'default'."
  )
  .option(
    "--ws-port <n>",
    "Override config.yml wsPort. Used by the supervisor to assign a per-session port from its allocation pool.",
    (v) => parseInt(v, 10)
  )
  .action((opts) => runCommand(opts));

program
  .command("supervisor")
  .description("Run the multi-session supervisor (default for `pnpm dev`)")
  .option("--config <path>", "Path to config.yml")
  .action(async (opts) => {
    const { supervisorCommand } = await import("./commands/supervisor.js");
    await supervisorCommand(opts);
  });

program
  .command("status [runId]")
  .description("Show checkpoint status for a run (defaults to most recent)")
  .action((runId) => statusCommand(runId));

program
  .command("replay <runId>")
  .description("Replay a run from a specific checkpoint")
  .requiredOption("--from <checkpointId>", "Checkpoint id to start from")
  .action((runId, opts) => replayCommand(runId, opts));

program
  .command("history")
  .description("List all past runs")
  .action(() => historyCommand());

program
  .command("test-llm")
  .description("Ping the configured hosted LLM and print its raw response")
  .action(() => testLlmCommand());

program
  .command("pr-resolver")
  .description("Run the PR Resolver service (polls GitHub for tagged review comments and fixes them)")
  .option("--once", "Run a single poll cycle and exit (default: watch)")
  .option("--config <path>", "Path to config.yml")
  .action(async (opts) => {
    const { prResolverCommand } = await import("./commands/pr-resolver.js");
    await prResolverCommand(opts);
  });

const sessionsCmd = program
  .command("sessions")
  .description("Manage per-phase Claude CLI sessions (Phase 12 persistence)");

sessionsCmd
  .command("prune")
  .description(
    "Delete stale Claude session jsonl files under ~/.claude/projects/. Skips uuids referenced by active runs."
  )
  .option(
    "--older-than <duration>",
    'Cutoff age (e.g. "30d", "12h", "0d" for everything not active). Default 30d.',
    "30d"
  )
  .option("--dry-run", "Print what would be deleted without actually removing files")
  .action(async (opts) => {
    const { sessionsPruneCommand } = await import("./commands/sessions-prune.js");
    await sessionsPruneCommand(opts);
  });

registerParityCommands(program);

program.parseAsync(process.argv).catch((err) => {
  // eslint-disable-next-line no-console
  console.error(err);
  process.exit(1);
});
