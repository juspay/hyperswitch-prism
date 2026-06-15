import { spawn, type ChildProcess } from "node:child_process";
import { promises as fs, openSync, closeSync } from "node:fs";
import path from "node:path";
import net from "node:net";

/**
 * gRPC server lifecycle helpers used by the grpc-test checkpoint.
 *
 * Why this exists: the previous workflow asked the agent to start the server
 * inline via `cargo run --bin grpc-server &`. That pattern is unreliable when
 * driven from a remote agent's Bash tool — backgrounded children may not
 * survive across tool calls, and there is no log file the agent can read to
 * diagnose failures. Owning the server lifecycle in TypeScript:
 *   - guarantees pre-flight cleanup of stale processes,
 *   - gives the agent a known-good `localhost:8000`,
 *   - captures stdout+stderr to a log file the agent can Read,
 *   - guarantees cleanup on every exit path via the returned `kill` handle.
 */

export interface ServerHandle {
  pid: number;
  /** Best-effort SIGTERM, then SIGKILL after a grace period. Idempotent. */
  kill: () => Promise<void>;
}

export interface StartGrpcServerOptions {
  projectRoot: string;
  /** Absolute path; this function ensures the parent directory exists. */
  logFile: string;
  /**
   * Phase 10: gRPC listen port. Forwarded to cargo via env so parallel
   * sessions get distinct listeners. Defaults to 8000 when omitted for
   * callers that haven't been updated to the session-aware API.
   */
  grpcPort?: number;
  /**
   * Phase 10: dummy-connector HTTP port. Forwarded to cargo via env so
   * parallel sessions get distinct listeners. Defaults to 8080.
   */
  dummyConnectorPort?: number;
}

export interface WaitForHealthyOptions {
  host: string;
  port: number;
  timeoutMs: number;
  /** How often to retry the TCP probe. Default 500ms. */
  pollIntervalMs?: number;
}

export interface WaitForBuildOptions {
  /** Path to the cargo run log file (combined stdout+stderr). */
  logFile: string;
  /** Total budget for the build phase. Cold builds can run into the minutes. */
  timeoutMs: number;
  /** How often to read appended bytes from the log file. Default 1000ms. */
  pollIntervalMs?: number;
  /**
   * How often to emit a progress log line ("still compiling: <crate>") so the
   * dashboard shows the build isn't stuck. Default 30000ms. Set to 0 to mute.
   */
  progressIntervalMs?: number;
}

type Logger = (msg: string, level?: "info" | "warn" | "error") => void;

const noopLog: Logger = () => undefined;

async function runShell(
  cmd: string,
  log: Logger
): Promise<{ stdout: string; stderr: string; code: number | null }> {
  return new Promise((resolve) => {
    const child = spawn("bash", ["-c", cmd], {
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout!.on("data", (b: Buffer) => (stdout += b.toString("utf-8")));
    child.stderr!.on("data", (b: Buffer) => (stderr += b.toString("utf-8")));
    child.on("error", (err) => {
      log(`shell error: ${err.message}`, "warn");
      resolve({ stdout, stderr, code: null });
    });
    child.on("exit", (code) => resolve({ stdout, stderr, code }));
  });
}

/**
 * Kill anything bound to this session's gRPC + dummy-connector ports and any
 * leftover grpc-server binary processes from previous runs. Idempotent and
 * best-effort: if nothing is running, succeeds silently.
 *
 * Phase 10: ports are session-scoped. Without that scoping, session 2's
 * preflight would `lsof -ti:8000 | kill -9` and murder session 1's live
 * grpc-server (the actual bug the user hit running 3 sessions in parallel).
 *
 * Phase 11: dropped the global `pkill -9 -f 'target/debug/grpc-server'`.
 * Every worktree builds the binary at the same `target/debug/grpc-server`
 * path so pkill -f matched ALL sibling sessions' live servers — exactly
 * the cross-session murder the per-port scoping was supposed to prevent.
 * The two port-scoped lsof lines above remain authoritative; any orphan
 * binary still bound to one of our ports gets reaped, and orphans not
 * bound to our ports are harmless (won't compete).
 */
export async function killStaleProcesses(
  log: Logger = noopLog,
  grpcPort: number = 8000,
  dummyConnectorPort: number = 8080
): Promise<void> {
  // lsof emits non-zero when nothing matches; swallow with `|| true`.
  const cmds = [
    `lsof -ti:${grpcPort} | xargs -r kill -9 2>/dev/null || true`,
    `lsof -ti:${dummyConnectorPort} | xargs -r kill -9 2>/dev/null || true`,
  ];
  for (const cmd of cmds) {
    await runShell(cmd, log);
  }
  // Give the kernel a beat to release sockets before we try to bind again.
  await new Promise((r) => setTimeout(r, 500));
}

/**
 * Start `cargo run --bin grpc-server` in the project root, redirecting
 * stdout+stderr to `logFile`. Returns a handle with the pid and an idempotent
 * kill function. The child is *not* detached — it lives and dies with the
 * parent process so we never leak servers past a pipeline run.
 */
export async function startGrpcServer(
  opts: StartGrpcServerOptions,
  log: Logger = noopLog
): Promise<ServerHandle> {
  await fs.mkdir(path.dirname(opts.logFile), { recursive: true });
  // Truncate the log on each start so the agent reads only this run's output.
  const fd = openSync(opts.logFile, "w");
  const grpcPort = opts.grpcPort ?? 8000;
  const dummyConnectorPort = opts.dummyConnectorPort ?? 8080;
  try {
    // Phase 11: dropped GRPC_SERVER__SERVER__PORT / DUMMY_CONNECTOR__PORT
    // env overrides. The hyperswitch-prism binary loads its bind ports
    // from `config/development.toml` directly (see ucs_env::configs) and
    // ignores those keys, so they were dead code that gave a false sense
    // of per-session isolation. The real per-session port shift now
    // happens in preflight by templating `[server].port` and
    // `[metrics].port` in the worktree's development.toml before this
    // spawn runs; the grpc/dummy port options here are retained purely
    // so the kill block at the bottom of this function can scope its
    // `lsof -ti:${port}` reaps to this session's slot.
    const child: ChildProcess = spawn(
      "cargo",
      ["run", "--bin", "grpc-server"],
      {
        cwd: opts.projectRoot,
        stdio: ["ignore", fd, fd],
        env: process.env,
      }
    );

    if (typeof child.pid !== "number") {
      throw new Error("failed to spawn cargo run --bin grpc-server (no pid)");
    }

    log(`grpc-server started pid=${child.pid} log=${opts.logFile}`);

    let killed = false;
    const kill = async (): Promise<void> => {
      if (killed) return;
      killed = true;
      if (child.exitCode !== null || child.signalCode !== null) {
        return; // already dead
      }
      try {
        child.kill("SIGTERM");
      } catch {
        // process may have already exited
      }
      // Give cargo+server up to 3s to exit cleanly, then SIGKILL.
      const exited = await new Promise<boolean>((resolve) => {
        const timer = setTimeout(() => resolve(false), 3000);
        child.once("exit", () => {
          clearTimeout(timer);
          resolve(true);
        });
      });
      if (!exited) {
        try {
          child.kill("SIGKILL");
        } catch {
          // ignore
        }
      }
      // Also clean up the bound ports — `cargo run` spawns a child binary
      // (target/debug/grpc-server) which may outlive the cargo wrapper if
      // SIGTERM only reached the wrapper. Phase 10: scoped to this
      // session's slot so we don't murder a sibling session's server.
      await runShell(
        `lsof -ti:${grpcPort} | xargs -r kill -9 2>/dev/null || true`,
        log
      );
      await runShell(
        `lsof -ti:${dummyConnectorPort} | xargs -r kill -9 2>/dev/null || true`,
        log
      );
    };

    return { pid: child.pid, kill };
  } finally {
    closeSync(fd);
  }
}

/**
 * Watch the cargo run log file for the "Running `target/.../grpc-server`"
 * marker that cargo prints after `Finished` and immediately before exec'ing
 * the binary. Returning here means cargo is done compiling and the server
 * binary is launching — the TCP health probe (`waitForHealthy`) is the next
 * gate.
 *
 * Why this exists: a cold `cargo run --bin grpc-server` compiles the entire
 * dep graph (diesel, tokio, hyper, …) which can take 5-15 minutes. The TCP
 * probe's 45s budget is correct for "binary up → bind to port" but far too
 * short for "compile from scratch → bind to port." Splitting the wait in two
 * keeps each timer scoped to the right thing and lets us emit "still
 * compiling: <crate>" progress while the build runs.
 *
 * Detects compile failures fast: if `error[E…]` or
 * `error: could not compile` shows up in the log, throw immediately rather
 * than waiting for the outer timeout.
 */
export async function waitForBuildComplete(
  opts: WaitForBuildOptions,
  log: Logger = noopLog
): Promise<void> {
  const pollIntervalMs = opts.pollIntervalMs ?? 1000;
  const progressIntervalMs = opts.progressIntervalMs ?? 30_000;
  const deadline = Date.now() + opts.timeoutMs;
  const runningMarker = /Running\s+`?[^\n`]*grpc-server[^\n`]*`?/;
  const cargoErrorMarker = /(^error(\[E\d+\])?:|^error: could not compile)/m;

  let lastSize = 0;
  let buf = "";
  let lastProgressAt = Date.now();

  while (Date.now() < deadline) {
    try {
      const stat = await fs.stat(opts.logFile);
      if (stat.size > lastSize) {
        const fh = await fs.open(opts.logFile, "r");
        try {
          const slice = Buffer.alloc(stat.size - lastSize);
          await fh.read(slice, 0, slice.length, lastSize);
          buf += slice.toString("utf-8");
          lastSize = stat.size;
        } finally {
          await fh.close();
        }
      }
    } catch {
      // Log file not yet created by the cargo child — keep polling.
    }

    if (runningMarker.test(buf)) {
      log("grpc-server build complete; binary launching");
      return;
    }
    if (cargoErrorMarker.test(buf)) {
      const errLines = buf
        .split(/\r?\n/)
        .filter((l) => /^error/i.test(l))
        .slice(-5)
        .join("\n");
      throw new Error(
        `cargo build failed before grpc-server could start:\n${errLines}`
      );
    }

    if (
      progressIntervalMs > 0 &&
      Date.now() - lastProgressAt >= progressIntervalMs
    ) {
      const lastCompiling = buf
        .split(/\r?\n/)
        .reverse()
        .find((l) => /^\s*Compiling\b/.test(l));
      if (lastCompiling) {
        log(`still building: ${lastCompiling.trim()}`);
      }
      lastProgressAt = Date.now();
    }

    await new Promise((r) => setTimeout(r, pollIntervalMs));
  }

  throw new Error(
    `grpc-server build did not complete within ${opts.timeoutMs}ms; ` +
      `last log: ${buf.slice(-500) || "(empty)"}`
  );
}

/**
 * Resolve when a TCP connection to host:port succeeds. Used as a liveness
 * signal for the gRPC server — TCP-bound is sufficient because the very next
 * step is the agent's `grpcurl list`, which will surface any "service not
 * registered yet" issue with a clear error message.
 *
 * Rejects with the last connection error if `timeoutMs` elapses without
 * success.
 */
export async function waitForHealthy(
  opts: WaitForHealthyOptions,
  log: Logger = noopLog
): Promise<void> {
  const pollIntervalMs = opts.pollIntervalMs ?? 500;
  const deadline = Date.now() + opts.timeoutMs;
  let lastErr: Error = new Error(
    `grpc-server did not become healthy within ${opts.timeoutMs}ms`
  );

  while (Date.now() < deadline) {
    const ok = await new Promise<boolean>((resolve) => {
      const sock = net.createConnection({ host: opts.host, port: opts.port });
      const done = (success: boolean, err?: Error) => {
        sock.destroy();
        if (err) lastErr = err;
        resolve(success);
      };
      sock.once("connect", () => done(true));
      sock.once("error", (err) => done(false, err));
      sock.setTimeout(1000, () =>
        done(false, new Error("tcp connect timeout"))
      );
    });
    if (ok) {
      log(`grpc-server healthy at ${opts.host}:${opts.port}`);
      return;
    }
    await new Promise((r) => setTimeout(r, pollIntervalMs));
  }
  throw lastErr;
}

/**
 * Read up to the last `maxBytes` of a file. Returns "" if the file can't be
 * read (e.g. server crashed before any output). Used to attach server log
 * tails to error reports so the dashboard surfaces real diagnostics.
 */
export async function tailLogFile(
  logFile: string,
  maxBytes = 2048
): Promise<string> {
  try {
    const stat = await fs.stat(logFile);
    const start = Math.max(0, stat.size - maxBytes);
    const fh = await fs.open(logFile, "r");
    try {
      const buf = Buffer.alloc(stat.size - start);
      await fh.read(buf, 0, buf.length, start);
      return buf.toString("utf-8");
    } finally {
      await fh.close();
    }
  } catch {
    return "";
  }
}
