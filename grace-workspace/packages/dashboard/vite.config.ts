import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { spawn, type ChildProcess } from "node:child_process";
import { readFile, readdir, stat } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { IncomingMessage, ServerResponse } from "node:http";

const WORKSPACE_ROOT = resolve(__dirname, "..", "..");
const PARITY_CACHE_DIR = join(WORKSPACE_ROOT, ".cache");
const PARITY_DASHBOARD_MD = join(WORKSPACE_ROOT, "parity-dashboard.md");
const PARITY_CONNECTORS_DIR = join(WORKSPACE_ROOT, "connectors");
const CLI_DIST = resolve(WORKSPACE_ROOT, "packages/cli/dist/index.js");

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

        // ---------- READ-ONLY endpoints (unchanged) -----------------------
        if (path === "/api/parity/tree.json") {
          const treePath = await newestTreeFile();
          if (!treePath) {
            res.statusCode = 404;
            res.setHeader("content-type", "application/json");
            res.end(JSON.stringify({ error: "no parity tree cache found — run `10xgrace parity dashboard` first" }));
            return;
          }
          const st = await stat(treePath).catch(() => null);
          const raw = await readFile(treePath, "utf8");
          res.statusCode = 200;
          res.setHeader("content-type", "application/json");
          res.setHeader("x-parity-source", treePath);
          res.setHeader("x-parity-mtime", st ? new Date(st.mtimeMs).toISOString() : "");
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
          try { activeRun.child.kill("SIGTERM"); } catch { /* ignore */ }
          res.statusCode = 200;
          res.setHeader("content-type", "application/json");
          res.end(JSON.stringify({ ok: true, message: "SIGTERM sent" }));
          return;
        }

        next();
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), parityApiPlugin()],
  server: { port: 3141 },
});
