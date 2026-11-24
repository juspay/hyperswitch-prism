import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { once } from "node:events";
import { existsSync, mkdtempSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test, { after, before } from "node:test";

// tenxgraceHome() and the token file both resolve from HOME at call time.
process.env.HOME = mkdtempSync(join(tmpdir(), "grace-sec-home-"));
const TOKEN = "test-control-token";
const PORT = 34142;

const { preflightCheckpoint } = await import("../packages/core/dist/checkpoints/preflight.js");
const { SessionManager, SessionSupervisor, StateManager } = await import("../packages/core/dist/index.js");
const require = createRequire(new URL("../packages/core/package.json", import.meta.url));
const { WebSocket } = require("ws");

const git = (cwd, ...args) => execFileSync("git", args, { cwd, stdio: "pipe" });

function makeRepo() {
  const dir = mkdtempSync(join(tmpdir(), "grace-sec-repo-"));
  git(dir, "init", "-q", "-b", "main");
  git(dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "--allow-empty", "-m", "init");
  git(dir, "update-ref", "refs/remotes/origin/main", "HEAD");
  return dir;
}

/** Resolves to the role the server gave us: "open" or the rejecting HTTP status. */
function connect(query, headers = {}) {
  return new Promise((resolve) => {
    const ws = new WebSocket(`ws://127.0.0.1:${PORT}${query}`, { headers });
    ws.on("open", () => {
      ws.close();
      resolve("open");
    });
    ws.on("unexpected-response", (_req, res) => resolve(res.statusCode));
    ws.on("error", () => resolve("error"));
  });
}

/** Sends a control message over an authenticated dashboard socket and waits for `replyType`. */
function control(type, payload, replyType) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${PORT}?token=${TOKEN}`);
    ws.on("error", reject);
    ws.on("open", () => {
      ws.send(JSON.stringify({ type: "hello", payload: { role: "dashboard" } }));
      ws.send(JSON.stringify({ type, payload }));
    });
    ws.on("message", (raw) => {
      const msg = JSON.parse(raw.toString());
      if (msg.type === replyType) {
        ws.close();
        resolve(msg.payload);
      }
    });
  });
}

const preflightWith = (projectRoot, fields) =>
  preflightCheckpoint.run({
    runId: "run_abc123",
    log: () => undefined,
    artifacts: { task: { projectRoot, ...fields } },
  });

let supervisor;

before(async () => {
  process.env.TENXGRACE_WS_TOKEN = TOKEN;
  const state = new StateManager();
  supervisor = new SessionSupervisor(state, new SessionManager(state), PORT, {
    cliEntryPath: "/nonexistent",
  });
  await once(supervisor.wss, "listening");
});

after(() => {
  supervisor.wss.close();
  for (const client of supervisor.wss.clients) client.terminate();
  // The supervisor parks timers and signal handlers for its whole process life.
  setTimeout(() => process.exit(), 100).unref();
});

test("preflight never hands connector or payment method to a shell", async () => {
  const repo = makeRepo();
  const marker = join(repo, "pwned");
  for (const fields of [
    { targetConnectors: [`x;touch ${marker};`], paymentMethod: "card" },
    { targetConnectors: ["stripe"], paymentMethod: `$(touch ${marker})` },
  ]) {
    await preflightWith(repo, fields);
    assert.equal(existsSync(marker), false, JSON.stringify(fields));
  }
});

test("preflight fails malformed identifiers, including tasks persisted before this fix", async () => {
  const result = await preflightWith(makeRepo(), { targetConnectors: ["../etc"], paymentMethod: "card" });
  assert.equal(result.passed, false);
  assert.match(result.errors.join(), /targetConnectors/);
});

test("control socket binds loopback by default", () => {
  assert.equal(supervisor.wss.address().address, "127.0.0.1");
});

test("control socket rejects a client with no token", async () => {
  assert.equal(await connect(""), 401);
});

test("control socket rejects a wrong token", async () => {
  assert.equal(await connect("?token=nope"), 401);
});

test("control socket rejects a cross-site page even with the right token", async () => {
  assert.equal(await connect(`?token=${TOKEN}`, { Origin: "https://evil.example" }), 401);
});

test("control socket accepts the dashboard origin with the token", async () => {
  assert.equal(await connect(`?token=${TOKEN}`, { Origin: "http://localhost:3141" }), "open");
});

test("control socket accepts an engine client, which sends no Origin", async () => {
  assert.equal(await connect(`?token=${TOKEN}`), "open");
});

test("sessions:create rejects malformed identifiers before provisioning", async () => {
  const reply = await control(
    "sessions:create",
    {
      name: "t",
      sourcePath: makeRepo(),
      initialTask: { title: "t", targetConnectors: ["a;b"], paymentMethod: "card" },
    },
    "sessions:create:error"
  );
  assert.match(reply.error, /targetConnectors/);
});

test("sessions:create keeps accepting the identifiers the dashboard sends", async () => {
  const reply = await control(
    "sessions:create",
    {
      name: "t",
      sourcePath: makeRepo(),
      initialTask: { title: "t", targetConnectors: ["elavon_pg"], paymentMethod: "ApplePay" },
    },
    "sessions:created"
  );
  assert.ok(reply.session.sessionId);
});

test("generated token file is private to the owner", async () => {
  const { loadControlToken } = await import("../packages/core/dist/control-auth.js");
  const saved = process.env.TENXGRACE_WS_TOKEN;
  delete process.env.TENXGRACE_WS_TOKEN;
  const token = loadControlToken();
  process.env.TENXGRACE_WS_TOKEN = saved;
  assert.ok(token.length >= 32);
  assert.equal(statSync(join(process.env.HOME, ".tenxgrace", "ws-token")).mode & 0o077, 0);
});
