import type { Checkpoint } from "../types.js";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";

/**
 * Scaffold checkpoint — runs `grace/rulesbook/codegen/add_connector.sh`
 * with stdout/stderr streamed line-by-line to `ctx.log` so the dashboard
 * shows each script step (Validating environment, Detecting flows,
 * Updating proto, Updating domain types, ...) in real time.
 *
 * Only present in the `new-connector` pipeline (see
 * `buildPipeline()` in checkpoints/index.ts). Other workflows
 * (add-payment-method, generic) skip scaffolding entirely.
 *
 * Idempotent: invoked with `--force -y` so re-runs overwrite generated
 * files and skip already-applied protobuf / domain-types updates with
 * warnings instead of failing.
 */
export const scaffoldCheckpoint: Checkpoint = {
  id: "scaffold",
  name: "Scaffold connector files",
  description:
    "Run add_connector.sh to stamp connector boilerplate (proto, domain types, default impls, configs, field-probe)",
  retryFrom: "scaffold",
  timeout: 5 * 60 * 1000, // 5 min
  async run(ctx) {
    const task = ctx.artifacts.task;
    if (!task) {
      return { passed: false, errors: ["Missing task artifact"] };
    }

    const connector = task.targetConnectors?.[0];
    if (!connector || connector === "unknown") {
      ctx.log(
        "[scaffold] No target connector — skipping add_connector.sh",
        "warn"
      );
      return { passed: true, output: "no connector to scaffold" };
    }

    const baseUrl = task.baseUrl?.trim() || task.sandboxUrl?.trim();
    if (!baseUrl) {
      ctx.log(
        "[scaffold] No baseUrl/sandboxUrl on task — skipping add_connector.sh",
        "warn"
      );
      return { passed: true, output: "no baseUrl to scaffold" };
    }

    const projectRoot = task.projectRoot;
    const scriptPath = path.join(
      projectRoot,
      "grace",
      "rulesbook",
      "codegen",
      "add_connector.sh"
    );
    if (!existsSync(scriptPath)) {
      return {
        passed: false,
        errors: [`add_connector.sh not found at ${scriptPath}`],
      };
    }

    ctx.log(
      `[scaffold] Running add_connector.sh ${connector} ${baseUrl} --force -y`,
      "info"
    );

    return await new Promise((resolve) => {
      // spawn (not execSync) so we can stream stdout/stderr line-by-line
      // to ctx.log as the script runs, instead of swallowing it and
      // surfacing only the tail at the end.
      const child = spawn(
        "bash",
        [scriptPath, connector, baseUrl, "--force", "-y"],
        { cwd: projectRoot, stdio: ["ignore", "pipe", "pipe"] }
      );

      const recentLines: string[] = [];

      const streamLines = (
        stream: NodeJS.ReadableStream,
        level: "info" | "error"
      ) => {
        let buf = "";
        stream.setEncoding("utf-8");
        stream.on("data", (chunk: string) => {
          buf += chunk;
          let nl: number;
          while ((nl = buf.indexOf("\n")) !== -1) {
            const line = buf.slice(0, nl);
            buf = buf.slice(nl + 1);
            // Strip ANSI colour codes so the dashboard sees clean text.
            // The script uses \033[…m sequences for green/red/etc.
            // eslint-disable-next-line no-control-regex
            const clean = line.replace(/\x1b\[[0-9;]*m/g, "").trimEnd();
            if (clean.length === 0) continue;
            recentLines.push(clean);
            if (recentLines.length > 60) recentLines.shift();
            ctx.log(`[scaffold] ${clean}`, level);
          }
        });
        stream.on("end", () => {
          if (buf.trim().length > 0) {
            // eslint-disable-next-line no-control-regex
            const clean = buf.replace(/\x1b\[[0-9;]*m/g, "").trim();
            ctx.log(`[scaffold] ${clean}`, level);
          }
        });
      };

      streamLines(child.stdout!, "info");
      streamLines(child.stderr!, "error");

      child.on("error", (err) => {
        resolve({
          passed: false,
          errors: [`Failed to spawn add_connector.sh: ${err.message}`],
        });
      });

      child.on("close", (code) => {
        if (code === 0) {
          ctx.log("[scaffold] ✓ add_connector.sh completed", "success");
          resolve({ passed: true, output: recentLines.slice(-20).join("\n") });
        } else {
          resolve({
            passed: false,
            errors: [
              `add_connector.sh exited with code ${code}.\n${recentLines
                .slice(-20)
                .join("\n")}`,
            ],
          });
        }
      });
    });
  },
};
