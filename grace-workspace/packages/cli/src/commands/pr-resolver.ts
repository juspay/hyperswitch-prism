import {
  PrResolverService,
  loadConfig,
  onPrResolverEvent,
  setConfig,
} from "@byne/core";

interface PrResolverOpts {
  once?: boolean;
  config?: string;
}

/**
 * `byne pr-resolver` — runs the PR Resolver service standalone, without the
 * supervisor/dashboard. Useful for one-off cycles (`--once`) during dev or
 * a long-running watch loop in CI / a tmux pane.
 */
export async function prResolverCommand(opts: PrResolverOpts): Promise<void> {
  const cfg = loadConfig(opts.config);
  setConfig(cfg);

  // eslint-disable-next-line no-console
  console.log(`\x1b[1mByne · PR Resolver\x1b[0m`);
  // eslint-disable-next-line no-console
  console.log(`  repo:     ${cfg.prResolver.githubRepo || "\x1b[31m<not set>\x1b[0m"}`);
  // eslint-disable-next-line no-console
  console.log(`  trigger:  ${cfg.prResolver.trigger}`);
  // eslint-disable-next-line no-console
  console.log(`  interval: ${cfg.prResolver.pollInterval}s`);
  // eslint-disable-next-line no-console
  console.log(`  worktree: ${cfg.prResolver.worktreePath}`);
  // eslint-disable-next-line no-console
  console.log(`  prompts:  ${cfg.prResolver.promptsDir}`);
  // eslint-disable-next-line no-console
  console.log(`  state:    ${cfg.prResolver.stateFilePath}`);
  // eslint-disable-next-line no-console
  console.log("");

  if (!cfg.prResolver.githubRepo) {
    // eslint-disable-next-line no-console
    console.error(
      `\x1b[31m✕ prResolver.githubRepo is empty.\x1b[0m\n` +
        `  Set BYNE_PR_RESOLVER_GITHUB_REPO=owner/name in .env or add` +
        `\n  prResolver.githubRepo to config.yml.`
    );
    process.exit(1);
  }

  // Bridge events to stdout so the CLI is informative without a dashboard.
  const unsubscribe = onPrResolverEvent((event) => {
    const t = new Date(event.timestamp).toISOString();
    // eslint-disable-next-line no-console
    console.log(
      `\x1b[36m${t}\x1b[0m \x1b[35m${event.type}\x1b[0m ${formatPayload(event.payload)}`
    );
  });

  let service: PrResolverService;
  try {
    service = new PrResolverService(cfg.prResolver, cfg);
  } catch (err) {
    unsubscribe();
    // eslint-disable-next-line no-console
    console.error(`\x1b[31m✕ ${err instanceof Error ? err.message : String(err)}\x1b[0m`);
    process.exit(1);
  }

  // Cancellation: SIGINT / SIGTERM stop the next cycle cleanly.
  let stopping = false;
  const shutdown = (sig: string) => {
    if (stopping) return;
    stopping = true;
    // eslint-disable-next-line no-console
    console.log(`\n\x1b[33m[${sig}] cancelling — finishing current cycle…\x1b[0m`);
    service.cancel();
  };
  process.on("SIGINT", () => shutdown("SIGINT"));
  process.on("SIGTERM", () => shutdown("SIGTERM"));

  try {
    await service.initialize();
    if (opts.once) {
      const summary = await service.runOnce();
      // eslint-disable-next-line no-console
      console.log(
        `\n\x1b[32mCycle ${summary.cycle} done — total=${summary.total} fixed=${summary.fixed} failed=${summary.failed} skipped=${summary.skipped} queued=${summary.queued}\x1b[0m`
      );
    } else {
      // eslint-disable-next-line no-console
      console.log(
        `\x1b[32mWatching ${cfg.prResolver.githubRepo} every ${cfg.prResolver.pollInterval}s. Ctrl-C to stop.\x1b[0m\n`
      );
      await service.runForever();
    }
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(
      `\x1b[31m✕ PR Resolver failed: ${err instanceof Error ? err.stack ?? err.message : String(err)}\x1b[0m`
    );
    process.exitCode = 1;
  } finally {
    unsubscribe();
  }
  setTimeout(() => process.exit(process.exitCode ?? 0), 50).unref();
}

function formatPayload(payload: Record<string, unknown>): string {
  if (!payload || Object.keys(payload).length === 0) return "";
  // Compact one-line summary; truncate long values.
  const parts: string[] = [];
  for (const [key, value] of Object.entries(payload)) {
    if (value === undefined || value === null) continue;
    let stringified =
      typeof value === "string" ? value : JSON.stringify(value);
    if (stringified.length > 120) {
      stringified = stringified.slice(0, 117) + "…";
    }
    parts.push(`${key}=${stringified}`);
  }
  return parts.join(" ");
}
