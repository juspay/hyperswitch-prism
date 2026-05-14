import type { Checkpoint } from "../types.js";
import { execSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

export const preflightCheckpoint: Checkpoint = {
  id: "preflight",
  name: "Preflight Setup",
  description: "Create git branch and verify prerequisites for implementation",
  retryFrom: "preflight",
  timeout: 5 * 60 * 1000, // 5 min
  async run(ctx) {
    const task = ctx.artifacts.task;
    if (!task) {
      return { passed: false, errors: ["Missing task artifact"] };
    }

    const connector = task.targetConnectors?.[0] || "unknown";
    const flow = task.paymentMethod || "unknown";
    // Append the trailing 6-hex of ctx.runId so every run gets a unique branch
    // and `git checkout -b` below never collides with a leftover from a prior
    // aborted run. Also lets pr-review.ts cross-reference the branch back to
    // its originating run via ctx.artifacts.branch.
    const runSuffix = ctx.runId.slice(-6);
    const branchName = `feat/grace-${connector.toLowerCase()}-${flow.toLowerCase()}-${runSuffix}`;

    ctx.log("[preflight] Following Grace workflow: 1_orchestrator.md", "info");
    ctx.log(`[preflight] Creating branch: ${branchName}`, "info");

    try {
      // Check if we're in a git repo
      const projectRoot = task.projectRoot;
      const gitDir = path.join(projectRoot, ".git");
      if (!existsSync(gitDir)) {
        return { passed: false, errors: ["Not a git repository"] };
      }

      // Pull latest changes on current branch
      // ctx.log("[preflight] Pulling latest changes...", "info");
      // execSync("git pull", { cwd: projectRoot, stdio: "pipe" });

      // Create and checkout the new feature branch
      ctx.log(`[preflight] Creating and checking out branch: ${branchName}`, "info");
      execSync(`git checkout -b ${branchName}`, { cwd: projectRoot, stdio: "pipe" });

      // Verify creds.json exists. Phase 10: SessionManager.create()
      // symlinks TENXGRACE_CREDS_PATH into <projectRoot>/creds.json at session
      // creation, so for sessions created post-fix this resolves through
      // the symlink. Older sessions may still warn here.
      const credsPath = path.join(projectRoot, "creds.json");
      if (existsSync(credsPath)) {
        ctx.log("[preflight] Credentials file found", "info");
      } else {
        ctx.log(
          "[preflight] Warning: creds.json not found. Set TENXGRACE_CREDS_PATH in your env so future sessions get a symlink.",
          "warn"
        );
      }

      // Phase 11: rewrite the two `port = N` lines that govern this
      // worktree's grpc-server bind targets so concurrent sessions don't
      // fight for ports 8000/8080.
      //
      //   [server].port  → grpcPort  (8000 + slot — gRPC service)
      //   [metrics].port → metricsPort (8080 + slot — HTTP metrics/health)
      //
      // The regex captures `[section]` + everything-but-the-next-section-
      // header up through the first `port = ` line within that section,
      // then substitutes the integer. `[^\[]*?` is a non-greedy bound
      // limited to "no section-opening bracket" so a [server] match can't
      // accidentally rewrite the [metrics].port line, and vice versa.
      //
      // Snapshot the unmodified file once per worktree so the supervisor's
      // restoreSessionConfigs can return things to byte-identical state on
      // engine exit.
      //
      // NOTE: `task.dummyConnectorPort` is semantically the [metrics] port
      // in this codebase — kept under the Phase-10 field name to avoid a
      // wider rename. Renaming is a hygiene follow-up.
      const grpcPort = task.grpcPort;
      const metricsPort = task.dummyConnectorPort;
      if (grpcPort !== undefined || metricsPort !== undefined) {
        const devTomlPath = path.join(projectRoot, "config", "development.toml");
        if (existsSync(devTomlPath)) {
          try {
            const original = readFileSync(devTomlPath, "utf-8");
            const snapshotPath = devTomlPath + ".10xgrace-original";
            if (!existsSync(snapshotPath)) {
              writeFileSync(snapshotPath, original, "utf-8");
            }
            let updated = original;
            if (grpcPort !== undefined) {
              updated = updated.replace(
                /(\[server\][^[]*?\n\s*port\s*=\s*)\d+/,
                `$1${grpcPort}`
              );
            }
            if (metricsPort !== undefined) {
              updated = updated.replace(
                /(\[metrics\][^[]*?\n\s*port\s*=\s*)\d+/,
                `$1${metricsPort}`
              );
            }
            if (updated !== original) {
              writeFileSync(devTomlPath, updated, "utf-8");
              ctx.log(
                `[preflight] development.toml ports → server:${grpcPort} metrics:${metricsPort}`,
                "info"
              );
            }
          } catch (tomlErr) {
            // Non-fatal — log and continue. Default session (slot=0) uses
            // unshifted 8000/8080 either way; non-default sessions will
            // fail downstream with a more actionable error if their ports
            // weren't actually shifted.
            ctx.log(
              `[preflight] Could not template development.toml: ${tomlErr instanceof Error ? tomlErr.message : String(tomlErr)}`,
              "warn"
            );
          }
        }
      }

      ctx.log(`[preflight] ✓ Ready on branch: ${branchName}`, "success");

      return {
        passed: true,
        artifacts: {
          branch: branchName,
          projectRoot,
        },
      };
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      ctx.log(`[preflight] Failed: ${msg}`, "error");
      return {
        passed: false,
        errors: [`Preflight failed: ${msg}`],
      };
    }
  },
};
