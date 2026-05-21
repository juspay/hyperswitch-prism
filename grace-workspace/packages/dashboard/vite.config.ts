import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { spawn, type ChildProcess } from "node:child_process";
import { readFile, readdir, stat } from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import type { IncomingMessage, ServerResponse } from "node:http";

const WORKSPACE_ROOT = resolve(__dirname, "..", "..");
const PARITY_CACHE_DIR = join(WORKSPACE_ROOT, ".cache");
const PARITY_DASHBOARD_MD = join(WORKSPACE_ROOT, "parity-dashboard.md");
const PARITY_CONNECTORS_DIR = join(WORKSPACE_ROOT, "connectors");
const CLI_DIST = resolve(WORKSPACE_ROOT, "packages/cli/dist/index.js");
const DASHBOARD_STALE_MS = 5 * 60_000;

async function newestTreeFile(): Promise<string | null> {
  try {
    const files = await readdir(PARITY_CACHE_DIR);
    const trees = files.filter((f) => f.startsWith("tree-") && f.endsWith(".json"));
    if (trees.length === 0) return null;
    trees.sort((a, b) => (a < b ? 1 : -1));
    return join(PARITY_CACHE_DIR, trees[0]);
  } catch {
    return null;
  }
}

// Singleton: at most one in-flight force-refresh per dev server. Concurrent callers await the same promise.
let refreshInFlight: Promise<void> | null = null;

function forceRefreshTree(): Promise<void> {
  if (refreshInFlight) return refreshInFlight;
  refreshInFlight = new Promise<void>((resolveRefresh, rejectRefresh) => {
    const child = spawn(process.execPath, [CLI_DIST, "parity", "dashboard", "--force"], {
      cwd: WORKSPACE_ROOT,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stderr = "";
    child.stderr?.on("data", (b) => (stderr += b.toString()));
    child.on("error", (err) => rejectRefresh(err));
    child.on("close", (code) => {
      if (code === 0) resolveRefresh();
      else rejectRefresh(new Error(`parity dashboard --force exited ${code}: ${stderr.slice(-500)}`));
    });
  }).finally(() => {
    refreshInFlight = null;
  });
  return refreshInFlight;
}

// --- SSE bridge for autopilot heartbeats ----------------------------------
//
// At most one heartbeat in flight per dev server, matching the spec's
// parityAutopilot.heartbeat.maxInflightClaimed: 1 invariant. Subscribers
// (browser EventSource clients) attach via GET; POST kicks off the run.
// Backlog is replayed to late subscribers so a refresh re-joins the live run.

interface Subscriber {
  res: ServerResponse;
  alive: boolean;
}

interface RunState {
  leaf: number;
  dryRun: boolean;
  startedAt: number;
  child: ChildProcess;
  logTail: string[]; // SSE chunks, last 200 entries
  subscribers: Set<Subscriber>;
  done: boolean;
  exitCode: number | null;
}

let activeRun: RunState | null = null;

function broadcast(state: RunState, sse: string) {
  state.logTail.push(sse);
  if (state.logTail.length > 200) state.logTail.shift();
  for (const sub of state.subscribers) {
    if (!sub.alive) continue;
    try {
      sub.res.write(sse);
    } catch {
      sub.alive = false;
    }
  }
}

function feedLine(state: RunState, line: string) {
  if (!line) return;
  let event = "log";
  let data: unknown = line;
  const marker = "__PARITY_PROGRESS__ ";
  if (line.startsWith(marker)) {
    try {
      data = JSON.parse(line.slice(marker.length));
      event = "progress";
    } catch {
      // fall through as raw log
    }
  }
  const sse = `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;
  broadcast(state, sse);
}

function spawnHeartbeat(leaf: number, dryRun: boolean, runner?: "claude-code" | "opencode"): RunState {
  const args = [CLI_DIST, "parity", "tick", "--leaf", String(leaf)];
  if (dryRun) args.push("--dry-run");
  if (runner) args.push("--runner", runner);
  const child = spawn(process.execPath, args, {
    // Detach into a new process group so we can signal the whole tree
    // (claude grandchild included) via `kill(-pgid, ...)` on cancel.
    detached: true,
    cwd: WORKSPACE_ROOT,
    env: process.env,
    stdio: ["ignore", "pipe", "pipe"],
  });

  const state: RunState = {
    leaf,
    dryRun,
    startedAt: Date.now(),
    child,
    logTail: [],
    subscribers: new Set(),
    done: false,
    exitCode: null,
  };

  let stdoutBuf = "";
  let stderrBuf = "";
  child.stdout?.on("data", (buf: Buffer) => {
    stdoutBuf += buf.toString("utf8");
    let nl;
    while ((nl = stdoutBuf.indexOf("\n")) >= 0) {
      const line = stdoutBuf.slice(0, nl);
      stdoutBuf = stdoutBuf.slice(nl + 1);
      feedLine(state, line);
    }
  });
  child.stderr?.on("data", (buf: Buffer) => {
    stderrBuf += buf.toString("utf8");
    let nl;
    while ((nl = stderrBuf.indexOf("\n")) >= 0) {
      const line = stderrBuf.slice(0, nl);
      stderrBuf = stderrBuf.slice(nl + 1);
      feedLine(state, line);
    }
  });
  child.on("exit", (code, signal) => {
    if (stdoutBuf) feedLine(state, stdoutBuf);
    if (stderrBuf) feedLine(state, stderrBuf);
    state.done = true;
    state.exitCode = code;
    const sse = `event: done\ndata: ${JSON.stringify({ code, signal })}\n\n`;
    broadcast(state, sse);
    for (const sub of state.subscribers) {
      try { sub.res.end(); } catch { /* ignore */ }
    }
  });

  return state;
}

function attachSubscriber(state: RunState, res: ServerResponse): Subscriber {
  res.statusCode = 200;
  res.setHeader("content-type", "text/event-stream");
  res.setHeader("cache-control", "no-cache, no-transform");
  res.setHeader("connection", "keep-alive");
  res.setHeader("x-accel-buffering", "no");
  res.write(`event: hello\ndata: ${JSON.stringify({ leaf: state.leaf, dryRun: state.dryRun, startedAt: state.startedAt })}\n\n`);

  for (const chunk of state.logTail) res.write(chunk);

  const sub: Subscriber = { res, alive: true };
  state.subscribers.add(sub);

  if (state.done) {
    try { res.end(); } catch { /* ignore */ }
  }
  return sub;
}

async function readJsonBody(req: IncomingMessage): Promise<any> {
  return await new Promise((resolveBody) => {
    let buf = "";
    req.on("data", (c) => (buf += c));
    req.on("end", () => {
      if (!buf) return resolveBody({});
      try { resolveBody(JSON.parse(buf)); } catch { resolveBody({}); }
    });
  });
}

function parityApiPlugin(): Plugin {
  return {
    name: "parity-api",
    configureServer(server) {
      server.middlewares.use(async (req, res, next) => {
        if (!req.url) return next();
        const url = new URL(req.url, "http://localhost");
        const path = url.pathname;

        // ---------- READ-ONLY endpoints (with auto-stale refresh) ----------
        if (path === "/api/parity/tree.json") {
          const force = url.searchParams.get("force") === "1";
          let treePath = await newestTreeFile();
          let st = treePath ? await stat(treePath).catch(() => null) : null;
          const isMissing = !treePath;
          const isStale = !!st && Date.now() - st.mtimeMs > DASHBOARD_STALE_MS;
          let refreshError: string | null = null;
          if (force || isMissing || isStale) {
            try {
              await forceRefreshTree();
              treePath = await newestTreeFile();
              st = treePath ? await stat(treePath).catch(() => null) : null;
            } catch (err) {
              refreshError = (err as Error).message;
              // Fall through and serve the stale cache if we have one;
              // only 404 if there is truly no cache to serve.
            }
          }
          if (!treePath) {
            res.statusCode = 503;
            res.setHeader("content-type", "application/json");
            res.end(JSON.stringify({
              error: refreshError ?? "no parity tree cache; run `10xgrace parity dashboard` first",
            }));
            return;
          }
          const raw = await readFile(treePath, "utf8");
          res.statusCode = 200;
          res.setHeader("content-type", "application/json");
          res.setHeader("x-parity-source", treePath);
          res.setHeader("x-parity-mtime", st ? new Date(st.mtimeMs).toISOString() : "");
          if (refreshError) res.setHeader("x-parity-refresh-error", refreshError.slice(0, 200));
          res.end(raw);
          return;
        }

        if (path === "/api/parity/dashboard.md") {
          try {
            const raw = await readFile(PARITY_DASHBOARD_MD, "utf8");
            res.statusCode = 200;
            res.setHeader("content-type", "text/markdown");
            res.end(raw);
          } catch {
            res.statusCode = 404;
            res.end("# Parity dashboard not generated yet\n\nRun `10xgrace parity dashboard` first.");
          }
          return;
        }

        const connMatch = path.match(/^\/api\/parity\/connectors\/([a-z0-9_-]+)\.md$/);
        if (connMatch) {
          try {
            const raw = await readFile(join(PARITY_CONNECTORS_DIR, `${connMatch[1]}.md`), "utf8");
            res.statusCode = 200;
            res.setHeader("content-type", "text/markdown");
            res.end(raw);
          } catch {
            res.statusCode = 404;
            res.end(`# connector ${connMatch[1]} not found`);
          }
          return;
        }

        // ---------- CONFIG status -----------------------------------------
        if (path === "/api/parity/config" && req.method === "GET") {
          try {
            // Dynamic import keeps the plugin compatible with parity-core's
            // ESM dist and avoids circular import at vite config load time.
            const configModuleUrl = pathToFileURL(
              resolve(WORKSPACE_ROOT, "packages/parity-core/dist/config.js"),
            ).href;
            const mod = (await import(/* @vite-ignore */ configModuleUrl)) as {
              loadParityConfig: (explicitPath?: string) => any;
            };
            // Vite's cwd here is packages/dashboard; the base @10xgrace/core loader
            // resolves config.yml from process.cwd(), so we must pass the workspace
            // root explicitly or it won't find the config.
            const cfg = mod.loadParityConfig(resolve(WORKSPACE_ROOT, "config.yml"));
            res.statusCode = 200;
            res.setHeader("content-type", "application/json");
            res.end(JSON.stringify({
              prismPath: cfg.prismPath,
              oracleReadOnlyPath: cfg.oracleReadOnlyPath,
              bridgeWritePath: cfg.bridgeWritePath,
              hasOracle: Boolean(cfg.oracleReadOnlyPath),
              hasBridge: Boolean(cfg.bridgeWritePath),
              runner: cfg.llm?.runner ?? "claude-code",
              githubActor: cfg.github?.actor ?? "",
            }));
          } catch (e) {
            res.statusCode = 500;
            res.setHeader("content-type", "application/json");
            res.end(JSON.stringify({ error: (e as Error).message }));
          }
          return;
        }

        // ---------- LOCK status -------------------------------------------
        if (path === "/api/parity/lock" && req.method === "GET") {
          res.statusCode = 200;
          res.setHeader("content-type", "application/json");
          if (activeRun && !activeRun.done) {
            res.end(JSON.stringify({
              busy: true,
              leaf: activeRun.leaf,
              dryRun: activeRun.dryRun,
              startedAt: activeRun.startedAt,
            }));
          } else {
            res.end(JSON.stringify({ busy: false }));
          }
          return;
        }

        // ---------- START a heartbeat -------------------------------------
        const runStart = path.match(/^\/api\/parity\/run\/(\d+)$/);
        if (runStart && req.method === "POST") {
          const leaf = parseInt(runStart[1], 10);
          const body = await readJsonBody(req);
          const dryRun = body.dryRun !== false; // default true (safety)
          const runner: "claude-code" | "opencode" | undefined =
            body.runner === "claude-code" || body.runner === "opencode" ? body.runner : undefined;

          if (activeRun && !activeRun.done) {
            res.statusCode = 409;
            res.setHeader("content-type", "application/json");
            res.end(JSON.stringify({
              error: "another heartbeat is running",
              current: { leaf: activeRun.leaf, dryRun: activeRun.dryRun, startedAt: activeRun.startedAt },
            }));
            return;
          }

          activeRun = spawnHeartbeat(leaf, dryRun, runner);
          res.statusCode = 202;
          res.setHeader("content-type", "application/json");
          res.end(JSON.stringify({
            ok: true,
            leaf,
            dryRun,
            runner: runner ?? "(config default)",
            startedAt: activeRun.startedAt,
            streamUrl: `/api/parity/stream/${leaf}`,
          }));
          return;
        }

        // ---------- STREAM via SSE (GET, EventSource-friendly) ------------
        const streamMatch = path.match(/^\/api\/parity\/stream\/(\d+)$/);
        if (streamMatch && req.method === "GET") {
          const leaf = parseInt(streamMatch[1], 10);
          if (!activeRun || activeRun.leaf !== leaf) {
            res.statusCode = 404;
            res.setHeader("content-type", "application/json");
            res.end(JSON.stringify({ error: `no active run for leaf #${leaf}` }));
            return;
          }
          const sub = attachSubscriber(activeRun, res);
          req.on("close", () => {
            sub.alive = false;
            activeRun?.subscribers.delete(sub);
          });
          return;
        }

        // ---------- CANCEL ------------------------------------------------
        if (path === "/api/parity/cancel" && req.method === "POST") {
          if (!activeRun || activeRun.done) {
            res.statusCode = 200;
            res.setHeader("content-type", "application/json");
            res.end(JSON.stringify({ ok: true, message: "no active run" }));
            return;
          }
          // Spawned with `detached: true` so the child is the leader of its own
          // process group. Sending the signal to -pid reaches every descendant
          // (e.g. the `claude` grandchild that ignores SIGTERM to its parent).
          const pid = activeRun.child.pid;
          try {
            if (pid) {
              try { process.kill(-pid, "SIGTERM"); } catch { activeRun.child.kill("SIGTERM"); }
              // Escalate to SIGKILL after 5s if the group is still alive.
              setTimeout(() => {
                if (!activeRun || activeRun.done) return;
                try { process.kill(-pid, "SIGKILL"); } catch { /* already gone */ }
              }, 5000);
            } else {
              activeRun.child.kill("SIGTERM");
            }
          } catch { /* ignore */ }
          res.statusCode = 200;
          res.setHeader("content-type", "application/json");
          res.end(JSON.stringify({ ok: true, message: "SIGTERM sent to process group" }));
          return;
        }

        next();
      });
    },
  };
}

// --- Connector discovery API ---------------------------------------------
//
// POST /api/discover-connector            { name } -> blocking; returns DiscoveryResult
// POST /api/discover-connector/cancel/:id -> aborts an in-flight call
//
// v1 is synchronous (single long-lived HTTP request, no SSE). The wizard
// shows a spinner in Step 2 while the request is in flight.

interface DiscoveryJobState {
  abortController: AbortController;
  startedAt: number;
  connectorName: string;
}

function connectorDiscoveryPlugin(): Plugin {
  const activeJobs = new Map<string, DiscoveryJobState>();

  return {
    name: "connector-discovery-api",
    configureServer(server) {
      server.middlewares.use(async (req, res, next) => {
        if (!req.url) return next();
        const url = new URL(req.url, "http://localhost");
        const path = url.pathname;

        // Cancel an in-flight discovery.
        const cancelMatch = path.match(/^\/api\/discover-connector\/cancel\/(.+)$/);
        if (cancelMatch && req.method === "POST") {
          const id = cancelMatch[1];
          const job = activeJobs.get(id);
          if (job) job.abortController.abort();
          res.statusCode = 200;
          res.setHeader("content-type", "application/json");
          res.end(JSON.stringify({ ok: true, jobId: id, cancelled: !!job }));
          return;
        }

        // Run discovery.
        if (path === "/api/discover-connector" && req.method === "POST") {
          let body: { name?: string; model?: string } = {};
          try {
            body = await readJsonBody(req);
          } catch {
            res.statusCode = 400;
            res.setHeader("content-type", "application/json");
            res.end(JSON.stringify({ error: "invalid JSON body" }));
            return;
          }
          const name = body.name?.trim();
          if (!name) {
            res.statusCode = 400;
            res.setHeader("content-type", "application/json");
            res.end(JSON.stringify({ error: "missing 'name'" }));
            return;
          }

          const jobId = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
          const abortController = new AbortController();
          activeJobs.set(jobId, {
            abortController,
            startedAt: Date.now(),
            connectorName: name,
          });

          // Lazy-import @10xgrace/core so vite.config.ts doesn't pay the cost
          // until someone actually triggers discovery, and ESM resolution
          // happens at request time (after `pnpm build` of core).
          try {
            const coreModuleUrl = pathToFileURL(
              resolve(WORKSPACE_ROOT, "packages/core/dist/index.js"),
            ).href;
            const mod = (await import(/* @vite-ignore */ coreModuleUrl)) as {
              discoverConnector: (opts: {
                connectorName: string;
                model?: string;
                onProgress?: (msg: string) => void;
              }) => Promise<unknown>;
            };

            const result = await mod.discoverConnector({
              connectorName: name,
              model: body.model,
              onProgress: (msg) => {
                // v1: log on the server side; client polling could pick this up later.
                process.stdout.write(`[discover ${jobId}] ${msg}\n`);
              },
            });

            activeJobs.delete(jobId);
            res.statusCode = 200;
            res.setHeader("content-type", "application/json");
            res.setHeader("x-job-id", jobId);
            res.end(JSON.stringify({ jobId, result }));
          } catch (err) {
            activeJobs.delete(jobId);
            res.statusCode = 500;
            res.setHeader("content-type", "application/json");
            res.end(JSON.stringify({
              jobId,
              error: (err as Error).message ?? String(err),
              cancelled: abortController.signal.aborted,
            }));
          }
          return;
        }

        next();
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), parityApiPlugin(), connectorDiscoveryPlugin()],
  server: { port: 3141 },
});
