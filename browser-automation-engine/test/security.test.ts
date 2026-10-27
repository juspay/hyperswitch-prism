import assert from "node:assert/strict";
import { test } from "node:test";
import dns from "node:dns/promises";
import http from "node:http";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { parseRunRequest } from "../src/utils/validation";
import { isPublicAddress, publicDestination, startNetworkGuard } from "../src/utils/networkGuard";
import { AutomationEngine } from "../src/engine/automationEngine";
import type { BrowserSession } from "../src/drivers/browserDriver";

const job = { url: "https://example.com/", rules: [] };

test("reject the submitted PoC's file, script and screenshot capabilities", () => {
  for (const url of ["file:///etc/passwd", "file:///proc/self/environ", "FILE:///tmp/creds.json", "data:text/html,secret", "javascript:alert(1)"]) {
    assert.throws(() => parseRunRequest({ ...job, url }));
    assert.throws(() => parseRunRequest({ ...job, rules: [{ action: "goto", url }] }));
  }
  for (const action of ["screenshot", "evaluate"]) {
    assert.throws(() => parseRunRequest({ ...job, rules: [{ action, path: "/tmp/leak.png", expression: "document.body.innerText" }] }));
  }
  assert.throws(() => parseRunRequest({ ...job, options: { screenshotDir: "/tmp" } }));
  assert.throws(() => parseRunRequest({ ...job, url: "https://user:secret@example.com/" }));
});

test("bound resource options and reject oversized or invalid jobs", () => {
  for (const options of [{ headless: false }, { navigationTimeoutMs: 1e9 }, { viewport: { width: 0, height: 768 } }, { viewport: { width: 1920, height: Infinity } }]) {
    assert.throws(() => parseRunRequest({ ...job, options }));
  }
  assert.throws(() => parseRunRequest({ ...job, rules: Array(101).fill({ action: "click", selector: "button" }) }));
  assert.throws(() => parseRunRequest({ ...job, formFields: { invalid: { secret: "value" } } }));
});

test("block private, metadata, reserved and mapped addresses", () => {
  for (const address of ["127.0.0.1", "10.1.2.3", "172.16.0.1", "192.168.0.1", "169.254.169.254", "100.64.0.1", "0.0.0.0", "224.0.0.1", "192.0.0.1", "::1", "fc00::1", "fe80::1", "::ffff:169.254.169.254", "2001:db8::1"]) {
    assert.equal(isPublicAddress(address), false, address);
  }
  assert.equal(isPublicAddress("93.184.216.34"), true);
  assert.equal(isPublicAddress("2606:4700:4700::1111"), true);
});

test("validate every DNS answer and pin the approved connection IP", async () => {
  const publicResolver = (async () => [{ address: "93.184.216.34", family: 4 }]) as unknown as typeof dns.lookup;
  const mixedResolver = (async () => [{ address: "93.184.216.34", family: 4 }, { address: "127.0.0.1", family: 4 }]) as unknown as typeof dns.lookup;
  const target = await publicDestination("https://example.com/path", publicResolver);
  assert.equal(target.address, "93.184.216.34");
  assert.equal(target.url.hostname, "example.com");
  await assert.rejects(publicDestination("https://example.com/", mixedResolver));
  await assert.rejects(publicDestination("http://2130706433/"));
  await assert.rejects(publicDestination("http://example.com:8000/", publicResolver));
});

test("network guard rejects HTTP and HTTPS loopback/metadata requests before connecting", async () => {
  let hits = 0;
  const fixture = http.createServer((_req, res) => { hits++; res.end("PRIVATE_FIXTURE"); });
  await new Promise<void>(resolve => fixture.listen(0, "127.0.0.1", resolve));
  const guard = await startNetworkGuard();
  const proxy = new URL(guard.server);
  try {
    const fixtureAddress = fixture.address();
    assert.ok(fixtureAddress && typeof fixtureAddress !== "string");
    for (const target of [`http://127.0.0.1:${fixtureAddress.port}/`, "http://127.0.0.1/", "http://169.254.169.254/latest/meta-data/", "http://[::ffff:127.0.0.1]/"]) {
      const status = await new Promise<number | undefined>((resolve, reject) => {
        const request = http.get({ hostname: proxy.hostname, port: proxy.port, path: target }, res => { res.resume(); resolve(res.statusCode); });
        request.on("error", reject);
      });
      assert.equal(status, 403);
    }
    const connectStatus = await new Promise<number | undefined>((resolve, reject) => {
      const request = http.request({ hostname: proxy.hostname, port: proxy.port, method: "CONNECT", path: "127.0.0.1:443" });
      request.once("connect", (response, socket) => { socket.destroy(); resolve(response.statusCode); });
      request.on("error", reject);
      request.end();
    });
    assert.equal(connectStatus, 403);
    assert.equal(hits, 0);
    assert.equal(guard.blocked(), true);
  } finally {
    await guard.close();
    await new Promise<void>(resolve => fixture.close(() => resolve()));
  }
});

test("POST redirect fields reach the browser in memory and cleanup runs on failure", async () => {
  const received: unknown[] = [];
  let closed = 0;
  const session = {
    submitForm: async (...args: unknown[]) => { received.push(args); },
    currentUrl: () => job.url,
    assertNetworkPolicy: () => undefined,
    close: async () => { closed++; },
    getText: async () => { throw new Error("FIXTURE_FAILURE"); }
  } as unknown as BrowserSession;
  const engine = new AutomationEngine({ createSession: async () => session });
  const formFields = { submit: "preserved", tricky: '<script> & "', token: "NON_SECRET_TEST_TOKEN" };
  const result = await engine.run(parseRunRequest({ ...job, formFields, rules: [{ action: "extract", selector: "body", as: "result" }] }));
  assert.equal(result.success, false);
  assert.deepEqual((received[0] as unknown[])[1], formFields);
  assert.equal(closed, 1);
});

test("CLI rejects file inputs/output flags and never echoes their contents", () => {
  for (const [args, input] of [
    [[], JSON.stringify({ ...job, url: "file:///tmp/DO_NOT_DISCLOSE" })],
    [["--output", "/tmp/DO_NOT_CREATE"], JSON.stringify(job)],
    [[], "{INVALID_DO_NOT_ECHO"],
    [[], "X".repeat(1_048_577)]
  ] as [string[], string][]) {
    const result = spawnSync(process.execPath, ["--import", "tsx", "src/cli.ts", ...args], { cwd: path.resolve(__dirname, ".."), input, encoding: "utf8", timeout: 10_000 });
    assert.equal(result.status, 1);
    assert.equal(result.stdout, "");
    assert.doesNotMatch(result.stderr, /DO_NOT_|INVALID_DO_NOT_ECHO/);
  }
});

test("launcher requests an ephemeral non-root container without mounts, ports or secret env", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "prism-worker-launcher-"));
  const metadata = path.join(directory, "args.json");
  const executable = path.join(directory, "docker");
  try {
    await writeFile(executable, `#!${process.execPath}\nconst fs=require('node:fs'); if(process.argv[2]==='run'){fs.writeFileSync(${JSON.stringify(metadata)},JSON.stringify(process.argv.slice(2)));process.stdin.resume();process.stdin.on('end',()=>console.log('{"success":true}'));}\n`, { mode: 0o700 });
    const result = spawnSync("bash", ["scripts/run-worker.sh"], {
      cwd: path.resolve(__dirname, ".."), input: JSON.stringify(job), encoding: "utf8", timeout: 10_000,
      env: { PATH: `${directory}:${process.env.PATH}`, UCS_BROWSER_CONTAINER_RUNTIME: "docker", CONNECTOR_AUTH_FILE_PATH: "/tmp/PRIVATE_CREDS_FIXTURE", SECRET_FIXTURE: "DO_NOT_PASS" }
    });
    assert.equal(result.status, 0, result.stderr);
    const args: string[] = JSON.parse(await readFile(metadata, "utf8"));
    for (const expected of ["--rm", "--read-only", "--cap-drop=ALL", "--user", "10001:10001"]) assert.ok(args.includes(expected), expected);
    assert.ok(!args.some(arg => /--mount|--volume|--publish|--privileged|host:|PRIVATE_CREDS|SECRET_FIXTURE|DO_NOT_PASS/.test(arg)));
  } finally { await rm(directory, { recursive: true, force: true }); }
});
