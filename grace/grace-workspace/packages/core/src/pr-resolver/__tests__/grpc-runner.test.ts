import { afterEach, describe, expect, it } from "vitest";
import { createServer } from "node:net";
import { GrpcServerProcess } from "../grpc-runner.js";

/**
 * These tests exercise the spawn → probe → stop pipeline of
 * `GrpcServerProcess` without touching cargo. The `cargoBin` /
 * `cargoArgs` seam lets us substitute Node itself as the "cargo
 * binary" and feed it an inline script that simulates different
 * server behaviors (healthy, crashes immediately, never binds).
 */

const FAKE_LISTENER_SCRIPT = `
const net = require('node:net');
const port = Number(process.argv[1]);
const server = net.createServer((sock) => sock.end());
server.listen(port, '127.0.0.1', () => {
  process.stderr.write('FAKE_GRPC_LISTENING:' + port + '\\n');
});
const keepalive = setInterval(() => {}, 1000);
const shutdown = () => { clearInterval(keepalive); server.close(() => process.exit(0)); };
process.on('SIGTERM', shutdown);
process.on('SIGINT', shutdown);
`;

const CRASHING_SCRIPT = `
process.stderr.write('boom: simulated compile failure\\n');
process.stderr.write('error[E0001]: cannot find type \`Foo\` in scope\\n');
process.exit(1);
`;

const NEVER_LISTEN_SCRIPT = `
process.stderr.write('warming up but never binding\\n');
const keepalive = setInterval(() => {}, 1000);
process.on('SIGTERM', () => { clearInterval(keepalive); process.exit(0); });
process.on('SIGINT', () => { clearInterval(keepalive); process.exit(0); });
`;

async function getFreePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const srv = createServer();
    srv.unref();
    srv.on("error", reject);
    srv.listen(0, "127.0.0.1", () => {
      const addr = srv.address();
      const port = typeof addr === "object" && addr !== null ? addr.port : 0;
      srv.close(() => resolve(port));
    });
  });
}

describe("GrpcServerProcess", () => {
  const running: GrpcServerProcess[] = [];

  afterEach(async () => {
    while (running.length) {
      const p = running.pop();
      if (p) await p.stop().catch(() => undefined);
    }
  });

  function track(proc: GrpcServerProcess): GrpcServerProcess {
    running.push(proc);
    return proc;
  }

  it("resolves once the fake listener is healthy and stops cleanly", async () => {
    const port = await getFreePort();
    const probes: Array<{ attempt: number; ok: boolean }> = [];
    const stderrLines: string[] = [];
    const proc = track(
      new GrpcServerProcess({
        worktreePath: process.cwd(),
        port,
        cargoBin: process.execPath,
        cargoArgs: ["-e", FAKE_LISTENER_SCRIPT, String(port)],
        healthTimeoutMs: 10_000,
        onProbe: (attempt, ok) => probes.push({ attempt, ok }),
        onStderr: (line) => stderrLines.push(line),
      })
    );

    await proc.start();

    expect(probes.length).toBeGreaterThanOrEqual(1);
    expect(probes.some((p) => p.ok)).toBe(true);
    expect(stderrLines.some((l) => l.includes(`FAKE_GRPC_LISTENING:${port}`))).toBe(
      true
    );

    await proc.stop();
    // idempotent — calling stop twice should not throw.
    await proc.stop();
  });

  it("starting twice without stopping throws", async () => {
    const port = await getFreePort();
    const proc = track(
      new GrpcServerProcess({
        worktreePath: process.cwd(),
        port,
        cargoBin: process.execPath,
        cargoArgs: ["-e", FAKE_LISTENER_SCRIPT, String(port)],
        healthTimeoutMs: 10_000,
      })
    );
    await proc.start();
    await expect(proc.start()).rejects.toThrow(/already running/);
  });

  it("rejects with 'exited before becoming healthy' when the child crashes", async () => {
    const port = await getFreePort();
    let caught: unknown;
    try {
      await track(
        new GrpcServerProcess({
          worktreePath: process.cwd(),
          port,
          cargoBin: process.execPath,
          cargoArgs: ["-e", CRASHING_SCRIPT],
          healthTimeoutMs: 10_000,
        })
      ).start();
    } catch (err) {
      caught = err;
    }

    expect(caught).toBeInstanceOf(Error);
    const msg = (caught as Error).message;
    expect(msg).toMatch(/exited before becoming healthy/);
    // Last 15 stderr lines should be embedded in the error message.
    expect(msg).toMatch(/boom: simulated compile failure/);
    expect(msg).toMatch(/cannot find type/);
  });

  it("hits the configured health timeout when the binary never listens", async () => {
    const port = await getFreePort();
    const started = Date.now();
    let caught: unknown;
    try {
      await track(
        new GrpcServerProcess({
          worktreePath: process.cwd(),
          port,
          cargoBin: process.execPath,
          cargoArgs: ["-e", NEVER_LISTEN_SCRIPT],
          healthTimeoutMs: 1_500,
        })
      ).start();
    } catch (err) {
      caught = err;
    }
    const elapsed = Date.now() - started;

    expect(caught).toBeInstanceOf(Error);
    const msg = (caught as Error).message;
    expect(msg).toMatch(/didn't become healthy within/);
    expect(msg).toMatch(/prResolver\.grpcServerStartTimeoutMs/);
    expect(msg).toMatch(/warming up but never binding/);
    // Loop polls every 2s; the deadline check still fires after the first sleep.
    // Mainly we want to confirm we waited long enough for the timeout, not
    // that we returned instantly.
    expect(elapsed).toBeGreaterThanOrEqual(1_400);
  });

  it("mirrors stdout lines through onStderr (channel-merged for the dashboard)", async () => {
    const port = await getFreePort();
    const script = `
      process.stdout.write('   Compiling fake-grpc-server v0.1.0\\n');
      process.stdout.write('    Finished dev [unoptimized]\\n');
      const net = require('node:net');
      const server = net.createServer((s) => s.end());
      server.listen(${port}, '127.0.0.1', () => process.stderr.write('listening\\n'));
      const keepalive = setInterval(() => {}, 1000);
      process.on('SIGTERM', () => { clearInterval(keepalive); server.close(() => process.exit(0)); });
    `;
    const lines: string[] = [];
    const proc = track(
      new GrpcServerProcess({
        worktreePath: process.cwd(),
        port,
        cargoBin: process.execPath,
        cargoArgs: ["-e", script],
        healthTimeoutMs: 10_000,
        onStderr: (line) => lines.push(line),
      })
    );
    await proc.start();

    expect(lines.some((l) => l.includes("Compiling fake-grpc-server"))).toBe(true);
    expect(lines.some((l) => l.includes("Finished dev"))).toBe(true);
    expect(lines.some((l) => l.includes("listening"))).toBe(true);
  });

  it("passes env vars through to the child process", async () => {
    const port = await getFreePort();
    const script = `
      const sentinel = process.env.BYNE_TEST_SENTINEL || 'missing';
      process.stderr.write('SENTINEL=' + sentinel + '\\n');
      const net = require('node:net');
      const server = net.createServer((s) => s.end());
      server.listen(${port}, '127.0.0.1', () => process.stderr.write('ready\\n'));
      const keepalive = setInterval(() => {}, 1000);
      process.on('SIGTERM', () => { clearInterval(keepalive); server.close(() => process.exit(0)); });
    `;
    const lines: string[] = [];
    const proc = track(
      new GrpcServerProcess({
        worktreePath: process.cwd(),
        port,
        cargoBin: process.execPath,
        cargoArgs: ["-e", script],
        env: { BYNE_TEST_SENTINEL: "hello-from-test" },
        healthTimeoutMs: 10_000,
        onStderr: (line) => lines.push(line),
      })
    );
    await proc.start();

    expect(lines.some((l) => l.includes("SENTINEL=hello-from-test"))).toBe(true);
  });
});
