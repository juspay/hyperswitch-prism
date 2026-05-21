import type { Checkpoint } from "../types.js";
import { execSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
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

      // If the wizard supplied a pre-generated tech spec, write it to the
      // canonical grace location so L2_planning can short-circuit both its
      // LLM phases. We use the connector name as the filename (matching the
      // `generate-tech-spec` skill's output path).
      if (task.techSpecMarkdown && typeof task.techSpecMarkdown === "string") {
        const rawName = task.targetConnectors?.[0]?.trim();
        if (rawName) {
          const specDir = path.join(
            projectRoot,
            "grace",
            "rulesbook",
            "codegen",
            "references",
            "specs",
          );
          try {
            mkdirSync(specDir, { recursive: true });
            const specPath = path.join(specDir, `${rawName}.md`);
            writeFileSync(specPath, task.techSpecMarkdown, "utf-8");
            ctx.log(
              `[preflight] wrote pre-generated tech spec to ${specPath} (${task.techSpecMarkdown.length} chars)`,
              "info",
            );
          } catch (specErr) {
            // Non-fatal: L2 will fall back to its normal LLM-driven generation.
            const msg = specErr instanceof Error ? specErr.message : String(specErr);
            ctx.log(
              `[preflight] could not write tech spec: ${msg} (L2 will generate it instead)`,
              "warn",
            );
          }
        }
      }

      // Merge wizard-discovered verified URLs into the canonical grace 2.1
      // output file at `<projectRoot>/data/integration-source-links.json`.
      // Shape: `{ [ConnectorName]: ["url1", "url2", ...] }` — matches what
      // grace/workflow/2.1_links.md Phase 3 mandates. Preserves any pre-existing
      // entries for other connectors.
      if (
        task.discoveredConnectorUrls &&
        Array.isArray(task.discoveredConnectorUrls) &&
        task.discoveredConnectorUrls.length > 0
      ) {
        const rawName = task.targetConnectors?.[0]?.trim();
        if (rawName) {
          const dataDir = path.join(projectRoot, "data");
          const linksPath = path.join(dataDir, "integration-source-links.json");
          try {
            mkdirSync(dataDir, { recursive: true });
            let existing: Record<string, string[]> = {};
            if (existsSync(linksPath)) {
              try {
                const raw = readFileSync(linksPath, "utf-8");
                const parsed = JSON.parse(raw);
                if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
                  existing = parsed as Record<string, string[]>;
                }
              } catch {
                // If existing file is malformed, overwrite it rather than block.
              }
            }
            // Dedupe + preserve order.
            const seen = new Set<string>();
            const merged: string[] = [];
            for (const url of task.discoveredConnectorUrls) {
              if (typeof url !== "string") continue;
              const trimmed = url.trim();
              if (!trimmed || seen.has(trimmed)) continue;
              seen.add(trimmed);
              merged.push(trimmed);
            }
            existing[rawName] = merged;
            writeFileSync(linksPath, JSON.stringify(existing, null, 2), "utf-8");
            ctx.log(
              `[preflight] merged ${merged.length} verified URL(s) into ${linksPath} under "${rawName}"`,
              "info",
            );
          } catch (linksErr) {
            const msg = linksErr instanceof Error ? linksErr.message : String(linksErr);
            ctx.log(
              `[preflight] could not write integration-source-links.json: ${msg}`,
              "warn",
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
