import assert from "node:assert/strict";
import { test } from "node:test";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(__dirname, "..");
const runtime = process.env.UCS_BROWSER_CONTAINER_RUNTIME ?? "docker";
const image = process.env.UCS_BROWSER_WORKER_IMAGE ?? "prism-browser-worker:local";
const securityArgs = [
  "run", "--rm", "--interactive", "--pull=never", "--read-only", "--user", "10001:10001",
  "--cap-drop=ALL", "--security-opt", `seccomp=${path.join(root, "seccomp-profile.json")}`,
  "--pids-limit", "128", "--memory", "768m", "--cpus", "1", "--shm-size", "128m",
  "--tmpfs", "/tmp:rw,nosuid,nodev,size=256m,mode=1777", "--env", "HOME=/tmp"
];

function program(source: string, extraArgs: string[] = []): Record<string, unknown> {
  const result = spawnSync(runtime, [...securityArgs, ...extraArgs, "--entrypoint", "/usr/bin/setpriv", image, "--no-new-privs", "node", "-e", source], {
    encoding: "utf8", timeout: 60_000, maxBuffer: 1_048_576
  });
  assert.equal(result.status, 0, result.stderr || result.error?.message);
  return JSON.parse(result.stdout);
}

test("container enforces privilege restrictions and cannot see host credential/profile fixtures", () => {
  const entrypoint = spawnSync(runtime, ["image", "inspect", "--format", "{{json .Config.Entrypoint}}", image], { encoding: "utf8" });
  assert.equal(entrypoint.status, 0);
  assert.deepEqual(JSON.parse(entrypoint.stdout).slice(0, 2), ["/usr/bin/setpriv", "--no-new-privs"]);
  const directory = mkdtempSync(path.join(os.tmpdir(), "prism-secret-isolation-"));
  const fixture = path.join(directory, "creds.json");
  writeFileSync(fixture, '{"api_key":"HARMLESS_CREDENTIAL_FIXTURE"}', { mode: 0o600 });
  const previousPath = process.env.CONNECTOR_AUTH_FILE_PATH;
  process.env.CONNECTOR_AUTH_FILE_PATH = fixture;
  try {
    const result = program(`
      const fs = require('node:fs');
      const status = fs.readFileSync('/proc/self/status', 'utf8');
      let rootReadOnly = false;
      try { fs.writeFileSync('/app/must-not-write', 'fixture'); } catch (e) { rootReadOnly = e.code === 'EROFS' || e.code === 'EACCES'; }
      console.log(JSON.stringify({
        uid: process.getuid(),
        noNewPrivileges: /NoNewPrivs:\\s+1/.test(status),
        capabilitiesDropped: /CapEff:\\s+0+/.test(status),
        hostFileVisible: fs.existsSync(${JSON.stringify(fixture)}),
        savedProfileVisible: fs.existsSync('/app/gpay/.webkit-profile/storage-state.json'),
        hostEnvironmentVisible: Object.hasOwn(process.env, 'CONNECTOR_AUTH_FILE_PATH'),
        rootReadOnly
      }));
    `);
    assert.deepEqual(result, { uid: 10001, noNewPrivileges: true, capabilitiesDropped: true, hostFileVisible: false, savedProfileVisible: false, hostEnvironmentVisible: false, rootReadOnly: true });
  } finally {
    if (previousPath === undefined) delete process.env.CONNECTOR_AUTH_FILE_PATH;
    else process.env.CONNECTOR_AUTH_FILE_PATH = previousPath;
    rmSync(directory, { recursive: true, force: true });
  }
});

test("real sandboxed Chromium preserves POST fields and handles callback redirects without network access", () => {
  const result = program(`
    const assert = require('node:assert/strict');
    const { chromium } = require('playwright');
    const { AutomationEngine } = require('./dist/engine/automationEngine');
    const { PlaywrightDriverFactory } = require('./dist/drivers/playwrightDriver');
    const { parseRunRequest } = require('./dist/utils/validation');
    let postData;
    let launchOptions;
    let sandboxEnabled = false;
    const originalLaunch = chromium.launch.bind(chromium);
    chromium.launch = async options => {
      launchOptions = options;
      const browser = await originalLaunch(options);
      const cdp = await browser.newBrowserCDPSession();
      const command = await cdp.send('Browser.getBrowserCommandLine');
      sandboxEnabled = !command.arguments.includes('--no-sandbox');
      const createContext = browser.newContext.bind(browser);
      browser.newContext = async options => {
        const context = await createContext(options);
        const route = context.route.bind(context);
        context.route = (pattern, handler) => route(pattern, async request => {
          const url = new URL(request.request().url());
          if (url.hostname === 'fixture.invalid') {
            if (url.pathname === '/start') {
              await request.fulfill({ status: 302, headers: { location: 'https://callback.invalid/complete?marker=CALLBACK_OK' } });
            } else if (url.pathname === '/private-redirect') {
              await request.fulfill({ status: 302, headers: { location: 'http://169.254.169.254/latest/meta-data/' } });
            } else {
              assert.equal(request.request().method(), 'POST');
              postData = Object.fromEntries(new URLSearchParams(request.request().postData()));
              await request.fulfill({ contentType: 'text/html', body: '<h1>POST_OK</h1>' });
            }
          } else await handler(request);
        });
        return context;
      };
      return browser;
    };
    (async () => {
      const factory = new PlaywrightDriverFactory();
      const engine = new AutomationEngine(factory);
      try {
        const fields = { submit: 'field-does-not-shadow-method', tricky: '<script> & "', token: 'HARMLESS_FIXTURE' };
        const post = await engine.run(parseRunRequest({ url: 'https://fixture.invalid/post', formFields: fields, rules: [{ action: 'extract', selector: 'h1', as: 'heading' }] }));
        assert.equal(post.success, true, post.error);
        assert.deepEqual(postData, fields);
        assert.equal(post.data.heading, 'POST_OK');
        const callback = await engine.run(parseRunRequest({ url: 'https://fixture.invalid/start', callbackUrl: 'https://callback.invalid/complete', rules: [{ action: 'waitFor', urlContains: 'marker=CALLBACK_OK' }] }));
        assert.equal(callback.success, true, callback.error);
        assert.equal(callback.finalUrl, 'https://callback.invalid/complete?marker=CALLBACK_OK');
        const privateDestination = await engine.run(parseRunRequest({ url: 'http://169.254.169.254/latest/meta-data/', rules: [] }));
        assert.equal(privateDestination.success, false);
        const privateRedirect = await engine.run(parseRunRequest({ url: 'https://fixture.invalid/private-redirect', rules: [] }));
        assert.equal(privateRedirect.success, false);
        const session = await factory.createSession({ headless: true, slowMoMs: 0, defaultTimeoutMs: 1000, navigationTimeoutMs: 1000, viewport: { width: 800, height: 600 } });
        await assert.rejects(session.goto('file:///etc/passwd'));
        await session.close();
        assert.equal(launchOptions.chromiumSandbox, true);
        assert.equal(sandboxEnabled, true);
        console.log(JSON.stringify({ postFieldsPreserved: true, callbackRedirectHandled: true, metadataBlocked: true, privateRedirectBlocked: true, fileNavigationBlocked: true, chromiumSandboxEnabled: true }));
      } finally { await engine.cleanup(); }
    })().catch(error => { console.error(error.message); process.exitCode = 1; });
  `);
  assert.deepEqual(result, { postFieldsPreserved: true, callbackRedirectHandled: true, metadataBlocked: true, privateRedirectBlocked: true, fileNavigationBlocked: true, chromiumSandboxEnabled: true });
});

test("isolated launcher rejects PoC inputs, removes its container and publishes no service", () => {
  for (const request of [
    { url: "file:///etc/passwd", rules: [] },
    { url: "file:///proc/self/environ", rules: [] },
    { url: "https://example.com/", rules: [{ action: "evaluate", expression: "document.body.innerText" }] },
    { url: "https://example.com/", rules: [{ action: "screenshot", path: "/tmp/leak.png" }] }
  ]) {
    const name = `prism-worker-rejection-${process.pid}-${Math.random().toString(16).slice(2)}`;
    const result = spawnSync("bash", ["scripts/run-worker.sh"], {
      cwd: root, input: JSON.stringify(request), encoding: "utf8", timeout: 30_000,
      env: { ...process.env, UCS_BROWSER_CONTAINER_NAME: name }
    });
    assert.equal(result.status, 1, result.stderr);
    assert.equal(result.stdout, "");
    const inspect = spawnSync(runtime, ["inspect", name], { encoding: "utf8" });
    assert.notEqual(inspect.status, 0);
  }
});

test("deadline and cancellation remove the whole job container", async () => {
  for (const cancel of [false, true]) {
    const name = `prism-worker-lifetime-${process.pid}-${cancel ? "cancel" : "timeout"}`;
    const child = spawn("bash", ["scripts/run-worker.sh"], {
      cwd: root, stdio: ["pipe", "pipe", "pipe"],
      env: { ...process.env, UCS_BROWSER_CONTAINER_NAME: name, WORKER_TIMEOUT_MS: cancel ? "30000" : "1000" }
    });
    const completed = once(child, "close");
    try {
      // Keep stdin open. No job can start, and even this must have a deadline.
      if (cancel) {
        let running = false;
        for (let attempt = 0; attempt < 40; attempt++) {
          const inspected = spawnSync(runtime, ["inspect", "--format", "{{.State.Running}} {{json .HostConfig.PortBindings}}", name], { encoding: "utf8" });
          if (inspected.status === 0 && inspected.stdout.startsWith("true")) {
            assert.match(inspected.stdout, /(?:null|\{\})/);
            running = true;
            break;
          }
          await new Promise(resolve => setTimeout(resolve, 100));
        }
        assert.equal(running, true, "worker container did not start");
        child.kill("SIGTERM");
      }
      const outcome = await Promise.race([
        completed,
        new Promise<never>((_resolve, reject) => setTimeout(() => reject(new Error("Worker cleanup timed out")), 15_000).unref())
      ]);
      assert.equal(outcome[0], cancel ? 130 : 124);
      assert.notEqual(spawnSync(runtime, ["inspect", name], { encoding: "utf8" }).status, 0);
    } finally {
      child.stdin.destroy();
      child.kill("SIGKILL");
      spawnSync(runtime, ["rm", "--force", name], { stdio: "ignore" });
    }
  }
});

test("loss of the harness removes already-running browser jobs", { concurrency: 4 }, async t => {
  // Concurrent harness deaths exercise the race between the watchdog exiting
  // and the launcher's signal handler running while it waits for the worker.
  await Promise.all(Array.from({ length: 4 }, (_, attempt) => t.test(`crashed harness ${attempt + 1}`, async () => {
    const name = `prism-worker-parent-loss-${process.pid}-${attempt}`;
    const job = {
      url: "https://callback.invalid/complete",
      callbackUrl: "https://callback.invalid/complete",
      rules: [{ action: "waitFor", selector: "#never-created", timeoutMs: 60000 }]
    };
    const parent = spawn(process.execPath, ["-e", `
      const { spawn } = require('node:child_process');
      const worker = spawn('bash', ['scripts/run-worker.sh'], { stdio: ['pipe', 'ignore', 'ignore'] });
      worker.stdin.end(${JSON.stringify(JSON.stringify(job))});
      setInterval(() => {}, 1000);
    `], { cwd: root, stdio: "ignore", env: { ...process.env, UCS_BROWSER_CONTAINER_NAME: name } });
    const parentClosed = once(parent, "close");
    try {
      let browserRunning = false;
      for (let attempt = 0; attempt < 60; attempt++) {
        const processes = spawnSync(runtime, ["top", name, "-eo", "pid,args"], { encoding: "utf8" });
        if (/chromium_headless_shell/.test(processes.stdout)) { browserRunning = true; break; }
        await new Promise(resolve => setTimeout(resolve, 100));
      }
      assert.equal(browserRunning, true, "browser did not start");
      parent.kill("SIGKILL");
      await parentClosed;
      let removed = false;
      for (let attempt = 0; attempt < 60; attempt++) {
        if (spawnSync(runtime, ["inspect", name], { encoding: "utf8" }).status !== 0) { removed = true; break; }
        await new Promise(resolve => setTimeout(resolve, 100));
      }
      assert.equal(removed, true, "container survived its harness");
    } finally {
      parent.kill("SIGKILL");
      spawnSync(runtime, ["rm", "--force", name], { stdio: "ignore" });
    }
  })));
});
