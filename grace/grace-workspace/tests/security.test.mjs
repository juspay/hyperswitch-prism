import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, statSync, writeFileSync, chmodSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

// `ws` is a dependency of core, and pnpm does not hoist it to the workspace root.
const { WebSocket } = createRequire(new URL("../packages/core/package.json", import.meta.url))("ws");

// Point the token store at a throwaway home before anything reads it.
const home = mkdtempSync(join(tmpdir(), "grace-sec-home-"));
process.env.HOME = home;
delete process.env.TENXGRACE_CONTROL_TOKEN;
delete process.env.TENXGRACE_ALLOWED_HOSTS;
delete process.env.TENXGRACE_CONTROL_HOST;

const auth = await import("../packages/core/dist/control-auth.js");
const { validateTaskInputs, assertValidPort, isSafeScaffoldUrl } = await import(
  "../packages/core/dist/task-validation.js"
);
const { preflightCheckpoint } = await import("../packages/core/dist/checkpoints/preflight.js");
const { scaffoldCheckpoint } = await import("../packages/core/dist/checkpoints/scaffold.js");
const { killStaleProcesses } = await import("../packages/core/dist/checkpoints/grpc-server-lifecycle.js");
const { validateOverlay } = await import("../packages/core/dist/pr-resolver/runtime-config.js");
const { SessionSupervisor } = await import("../packages/core/dist/session-supervisor.js");

const INJECTIONS = (marker) => [
  `x;touch ${marker};`,
  `$(touch ${marker})`,
  `\`touch ${marker}\``,
  `x|touch ${marker}`,
  `x&&touch ${marker}`,
  `x\ntouch ${marker}`,
];

/** A repo with a local origin/main ref. Preflight tolerates the failed fetch. */
function gitRepoWithOrigin() {
  const root = mkdtempSync(join(tmpdir(), "grace-sec-repo-"));
  const work = join(root, "work");
  const git = (...args) => execFileSync("git", args, { cwd: work, stdio: "pipe" });
  mkdirSync(work);
  git("init", "--initial-branch=main");
  git("config", "user.email", "test@example.com");
  git("config", "user.name", "test");
  git("config", "commit.gpgsign", "false");
  writeFileSync(join(work, "README"), "x");
  git("add", "README");
  git("commit", "-m", "init");
  git("update-ref", "refs/remotes/origin/main", "HEAD");
  return { root, work };
}

function checkpointContext(task) {
  return { runId: "run_abcdef123456", log: () => {}, artifacts: { task }, task };
}

// ─── Injection through task fields ───────────────────────────────────────

for (const field of ["connector", "paymentMethod"]) {
  test(`preflight never executes shell syntax in ${field}`, async () => {
    const { root, work } = gitRepoWithOrigin();
    try {
      const marker = join(root, "pwned");
      for (const evil of INJECTIONS(marker)) {
        const task = {
          projectRoot: work,
          targetConnectors: [field === "connector" ? evil : "stripe"],
          paymentMethod: field === "paymentMethod" ? evil : "card",
        };
        const result = await preflightCheckpoint.run(checkpointContext(task));
        assert.equal(result.passed, false, JSON.stringify(evil));
        assert.equal(existsSync(marker), false, `executed: ${JSON.stringify(evil)}`);
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
}

test("preflight still creates the branch for valid names", async () => {
  const { root, work } = gitRepoWithOrigin();
  try {
    const task = { projectRoot: work, targetConnectors: ["AsiaPay"], paymentMethod: "apple_pay" };
    const result = await preflightCheckpoint.run(checkpointContext(task));
    assert.equal(result.passed, true, JSON.stringify(result.errors));
    const branch = execFileSync("git", ["branch", "--show-current"], { cwd: work }).toString().trim();
    assert.equal(branch, "feat/grace-asiapay-apple_pay-123456");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("validateTaskInputs accepts normal tasks", () => {
  for (const task of [
    undefined,
    {},
    { title: "anything; goes $(here)", description: "free text is not validated" },
    { targetConnectors: ["AsiaPay", "stripe_v2", "trust-pay"], paymentMethod: "apple_pay" },
    { targetConnectors: [""], paymentMethod: "" },
    { grpcPort: 8000, dummyConnectorPort: 8080, runnerModel: "claude-sonnet-4-5[1m]" },
    { runnerModel: "anthropic/claude-opus:latest" },
  ]) {
    assert.deepEqual(validateTaskInputs(task), [], JSON.stringify(task));
  }
});

test("validateTaskInputs rejects unsafe tasks", () => {
  for (const task of [
    "not an object",
    { targetConnectors: "stripe" },
    { targetConnectors: [42] },
    { targetConnectors: ["stripe", "$(id)"] },
    { targetConnectors: ["../../etc/passwd"] },
    { targetConnectors: ["__proto__"] },
    { targetConnectors: ["-flag"] },
    { paymentMethod: "a b" },
    { paymentMethod: { toLowerCase: "x" } },
    { paymentMethod: "x".repeat(65) },
    { grpcPort: "8000;id" },
    { grpcPort: 70000 },
    { dummyConnectorPort: "8080" },
    { dummyConnectorPort: 1.5 },
    { runnerModel: "--dangerous" },
    { runnerModel: "a b" },
  ]) {
    assert.notDeepEqual(validateTaskInputs(task), [], JSON.stringify(task));
  }
});

test("a port string never reaches the shell", async () => {
  const marker = join(home, "pwned-port");
  assert.throws(() => assertValidPort(`1;touch ${marker}`));
  await assert.rejects(killStaleProcesses(() => {}, `1;touch ${marker}`, 8080));
  await assert.rejects(killStaleProcesses(() => {}, 65000, `1;touch ${marker}`));
  assert.equal(existsSync(marker), false);
});

test("scaffold URLs cannot carry sed or shell syntax", async () => {
  for (const url of ["https://api.example.com", "https://x.example.com/v1?a=b&c=d", "http://localhost:8080/{{merchant}}"]) {
    assert.equal(isSafeScaffoldUrl(url), true, url);
  }
  for (const url of ["ftp://x", "--force", "https://x|e", "https://x y", "https://x'y", "https://x\"y", "https://$(id)", "https://x;y", "https://x\nid", "https://x\\y"]) {
    assert.equal(isSafeScaffoldUrl(url), false, url);
  }
  const root = mkdtempSync(join(tmpdir(), "grace-sec-scaffold-"));
  try {
    const dir = join(root, "grace", "rulesbook", "codegen");
    mkdirSync(dir, { recursive: true });
    const marker = join(root, "script-ran");
    writeFileSync(join(dir, "add_connector.sh"), `touch ${marker}\n`);
    chmodSync(join(dir, "add_connector.sh"), 0o755);
    const task = { projectRoot: root, targetConnectors: ["stripe"], sandboxUrl: "https://x|e" };
    const result = await scaffoldCheckpoint.run(checkpointContext(task));
    assert.equal(result.passed, false);
    assert.equal(existsSync(marker), false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("PR resolver overlay rejects bad ports and repos", () => {
  assert.equal(validateOverlay({ grpcPort: 8000, githubRepo: "juspay/hyperswitch-prism" }).ok, true);
  assert.equal(validateOverlay({ grpcPort: "1;id" }).ok, false);
  assert.equal(validateOverlay({ githubRepo: "--repo/x" }).ok, false);
  assert.equal(validateOverlay({ githubRepo: "a/b;id" }).ok, false);
});

// ─── Token storage ───────────────────────────────────────────────────────

test("control token is private, stable, and rejects weak overrides", () => {
  const token = auth.getControlToken({});
  assert.match(token, /^[0-9a-f]{64}$/);
  assert.equal(auth.getControlToken({}), token);
  assert.equal(statSync(auth.controlTokenPath()).mode & 0o777, 0o600);

  chmodSync(auth.controlTokenPath(), 0o644);
  assert.equal(auth.getControlToken({}), token);
  assert.equal(statSync(auth.controlTokenPath()).mode & 0o777, 0o600);

  assert.throws(() => auth.getControlToken({ TENXGRACE_CONTROL_TOKEN: "short" }));
  const custom = "a".repeat(40);
  assert.equal(auth.getControlToken({ TENXGRACE_CONTROL_TOKEN: custom }), custom);
});

test("an unusable token file is replaced", () => {
  const original = readFileSync(auth.controlTokenPath(), "utf-8");
  writeFileSync(auth.controlTokenPath(), "weak");
  const token = auth.getControlToken({});
  assert.match(token, /^[0-9a-f]{64}$/);
  assert.notEqual(token, original);
  assert.equal(auth.getControlToken({}), token);
});

// ─── Host and Origin rules ───────────────────────────────────────────────

test("host and origin allowlists", () => {
  for (const host of ["localhost:3141", "127.0.0.1:3141", "[::1]:3141", "localhost"]) {
    assert.equal(auth.isAllowedHostHeader(host, {}), true, host);
  }
  for (const host of [undefined, "", "evil.com", "evil.com:3141", "localhost.evil.com", "192.168.1.5:3141", "localhost@evil.com"]) {
    assert.equal(auth.isAllowedHostHeader(host, {}), false, String(host));
  }
  assert.equal(auth.isAllowedHostHeader("box.lan:3141", { TENXGRACE_ALLOWED_HOSTS: "other, BOX.lan" }), true);

  assert.equal(auth.isAllowedOrigin("http://localhost:3141", {}), true);
  for (const origin of ["https://evil.com", "http://localhost.evil.com", "null", "file://", "not a url"]) {
    assert.equal(auth.isAllowedOrigin(origin, {}), false, origin);
  }
});

test("local API rejects cross-site browser requests", () => {
  const host = "localhost:3141";
  const ok = (r) => assert.equal(auth.isAllowedLocalApiRequest({ host, ...r }), true, JSON.stringify(r));
  const no = (r) => assert.equal(auth.isAllowedLocalApiRequest({ host, ...r }), false, JSON.stringify(r));

  ok({ method: "GET" });
  ok({ method: "GET", secFetchSite: "same-origin" });
  ok({ method: "GET", secFetchSite: "none" });
  ok({ method: "POST", origin: "http://localhost:3141", secFetchSite: "same-origin" });

  no({ method: "POST" });
  no({ method: "POST", origin: "https://evil.com", secFetchSite: "cross-site" });
  no({ method: "POST", origin: "https://evil.com" });
  no({ method: "POST", origin: "http://localhost:9999" });
  no({ method: "POST", origin: "null" });
  no({ method: "GET", secFetchSite: "cross-site" });
  no({ method: "GET", secFetchSite: "same-site" });
});

// ─── The real supervisor socket ──────────────────────────────────────────

/** Minimal state double: the socket behaviour under test doesn't touch SQLite. */
function fakeState() {
  const calls = [];
  return {
    calls,
    listSessions: () => [{ sessionId: "s1", projectRoot: "/secret/location" }],
    getSession: () => null,
    updateSessionRuntime: () => {},
    recoverStaleSessions: () => {},
    getActiveSessions: () => [],
  };
}

function connect(port, { protocols, origin } = {}) {
  return new Promise((resolve) => {
    const frames = [];
    const ws = new WebSocket(`ws://127.0.0.1:${port}`, protocols ?? [], origin ? { origin } : {});
    ws.on("message", (raw) => frames.push(JSON.parse(raw.toString())));
    ws.on("open", () => resolve({ opened: true, ws, frames }));
    ws.on("error", () => resolve({ opened: false, ws, frames }));
  });
}

test("supervisor socket requires the token and sets a safe default bind", async (t) => {
  const port = 34000 + (process.pid % 1000);
  const created = [];
  const sessions = { create: async (input) => { created.push(input); return { sessionId: "new" }; } };
  const supervisor = new SessionSupervisor(fakeState(), sessions, port, { cliEntryPath: "/nonexistent" });
  t.after(() => {
    clearInterval(supervisor.reapTimer);
    supervisor.wss.close();
  });
  await new Promise((r) => (supervisor.wss.address() ? r() : supervisor.wss.once("listening", r)));

  assert.equal(supervisor.wss.address().address, "127.0.0.1");

  const token = auth.getControlToken();
  const good = auth.controlSocketProtocols(token);

  // Anonymous, wrong token, token-less protocol, hostile page holding the token.
  assert.equal((await connect(port)).opened, false);
  assert.equal((await connect(port, { protocols: auth.controlSocketProtocols("b".repeat(64)) })).opened, false);
  assert.equal((await connect(port, { protocols: [auth.CONTROL_PROTOCOL] })).opened, false);
  assert.equal((await connect(port, { protocols: good, origin: "https://evil.com" })).opened, false);

  // Legitimate clients.
  const engine = await connect(port, { protocols: good });
  assert.equal(engine.opened, true);
  assert.equal(engine.ws.protocol, auth.CONTROL_PROTOCOL);
  engine.ws.close();

  const dash = await connect(port, { protocols: good, origin: "http://localhost:3141" });
  assert.equal(dash.opened, true);
  dash.ws.send(JSON.stringify({ type: "hello", payload: { role: "dashboard" } }));
  await new Promise((r) => setTimeout(r, 100));
  assert.ok(dash.frames.some((f) => f.type === "sessions:snapshot"));

  // Even an authenticated client can't store an unsafe task.
  dash.ws.send(JSON.stringify({
    type: "sessions:create",
    payload: { name: "x", sourcePath: "/tmp", initialTask: { targetConnectors: ["$(id)"] } },
  }));
  await new Promise((r) => setTimeout(r, 100));
  assert.equal(created.length, 0);
  assert.ok(dash.frames.some((f) => f.type === "sessions:create:error"));

  dash.ws.send(JSON.stringify({
    type: "sessions:create",
    payload: { name: "x", sourcePath: "/tmp", initialTask: { targetConnectors: ["stripe"], paymentMethod: "card" } },
  }));
  await new Promise((r) => setTimeout(r, 100));
  assert.equal(created.length, 1);
  dash.ws.close();
});

test.after(() => rmSync(home, { recursive: true, force: true }));
