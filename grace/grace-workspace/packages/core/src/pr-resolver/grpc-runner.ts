import { type ChildProcess, spawn } from "node:child_process";
import { connect } from "node:net";
import { execa } from "execa";

/**
 * Spawns `cargo run -p grpc-server` in the worktree and waits for the
 * server to answer the gRPC reflection list before returning. Used for
 * the gRPC verification step that runs after build/clippy pass.
 *
 * MVP scope: one server per test step. Started and torn down for every
 * resolve cycle. Future work: keep it warm across retries.
 */

const DEFAULT_HEALTH_TIMEOUT_MS = 10 * 60 * 1000; // 10 min — cold cargo build
const HEALTH_POLL_MS = 2_000;
const STOP_GRACE_MS = 5_000;
const STDERR_TAIL_CAP = 60;
const DEFAULT_CARGO_BIN = "cargo";

export interface GrpcServerOptions {
  worktreePath: string;
  port: number;
  cargoBin?: string;
  /**
   * Args passed to `cargoBin`. Defaults to `["run", "--bin", "grpc-server"]`.
   * Tests override this to point at a fake binary that just opens a TCP
   * listener — exercises the spawn + probe + stop pipeline without
   * involving cargo or the real grpc-server.
   */
  cargoArgs?: string[];
  /** Extra env passed through, e.g. RUST_LOG, BYNE creds. */
  env?: NodeJS.ProcessEnv;
  /** Wall-clock budget for spawn + compile + first healthy probe. */
  healthTimeoutMs?: number;
  /** Called with each line of server stderr for log streaming. */
  onStderr?: (line: string) => void;
  /** Called once per `grpcurl list` probe attempt. */
  onProbe?: (attempt: number, ok: boolean) => void;
}

export class GrpcServerProcess {
  private child: ChildProcess | null = null;
  private exited = false;
  private exitReason: string | null = null;
  /** Rolling tail of stderr lines; included in the timeout error message. */
  private stderrTail: string[] = [];

  constructor(private readonly opts: GrpcServerOptions) {}

  async start(): Promise<void> {
    if (this.child) {
      throw new Error("GrpcServerProcess.start: already running");
    }
    // `--bin grpc-server` matches Byne's grpc-server-lifecycle checkpoint
    // (the historically-working pattern). `-p grpc-server` works too for a
    // single-bin package, but `--bin` is unambiguous if a future Cargo.toml
    // adds an extra binary target. Tests override via `cargoArgs`.
    const args = this.opts.cargoArgs ?? ["run", "--bin", "grpc-server"];
    const child = spawn(this.opts.cargoBin ?? DEFAULT_CARGO_BIN, args, {
      cwd: this.opts.worktreePath,
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, ...(this.opts.env ?? {}) },
      detached: false,
    });
    this.child = child;
    let stderrBuf = "";
    child.stderr?.on("data", (chunk: Buffer) => {
      stderrBuf += chunk.toString("utf8");
      let idx;
      while ((idx = stderrBuf.indexOf("\n")) >= 0) {
        const line = stderrBuf.slice(0, idx);
        stderrBuf = stderrBuf.slice(idx + 1);
        this.stderrTail.push(line);
        if (this.stderrTail.length > STDERR_TAIL_CAP) {
          this.stderrTail.shift();
        }
        this.opts.onStderr?.(line);
      }
    });
    // Cargo writes "Compiling …" to stdout too, so mirror it through onStderr
    // for the dashboard's live view; users don't need to know the channel
    // distinction.
    let stdoutBuf = "";
    child.stdout?.on("data", (chunk: Buffer) => {
      stdoutBuf += chunk.toString("utf8");
      let idx;
      while ((idx = stdoutBuf.indexOf("\n")) >= 0) {
        const line = stdoutBuf.slice(0, idx);
        stdoutBuf = stdoutBuf.slice(idx + 1);
        this.stderrTail.push(line);
        if (this.stderrTail.length > STDERR_TAIL_CAP) {
          this.stderrTail.shift();
        }
        this.opts.onStderr?.(line);
      }
    });
    child.on("exit", (code, signal) => {
      this.exited = true;
      this.exitReason = signal ?? (code !== null ? `code=${code}` : "unknown");
      this.child = null;
    });

    await this.waitForHealthy();
  }

  private async waitForHealthy(): Promise<void> {
    const timeoutMs = this.opts.healthTimeoutMs ?? DEFAULT_HEALTH_TIMEOUT_MS;
    const deadline = Date.now() + timeoutMs;
    let attempt = 0;
    while (Date.now() < deadline) {
      if (this.exited) {
        const tail = this.stderrTail.slice(-15).join("\n");
        throw new Error(
          `grpc-server exited before becoming healthy (${this.exitReason ?? "?"}).` +
            (tail ? `\n\nLast output:\n${tail}` : "")
        );
      }
      attempt += 1;
      const ok = await this.probe();
      this.opts.onProbe?.(attempt, ok);
      if (ok) return;
      await sleep(HEALTH_POLL_MS);
    }
    const tail = this.stderrTail.slice(-15).join("\n");
    await this.stop().catch(() => undefined);
    throw new Error(
      `grpc-server didn't become healthy within ${Math.round(timeoutMs / 1000)}s ` +
        `(port ${this.opts.port}). Bump prResolver.grpcServerStartTimeoutMs from the dashboard ` +
        `if your machine needs longer.` +
        (tail ? `\n\nLast output:\n${tail}` : "")
    );
  }

  /**
   * Health probe via raw TCP connect. We used to shell out to
   * `grpcurl … list`, but that requires gRPC reflection to be enabled on
   * the server — if it's compiled out or disabled, the probe never
   * succeeds even though the binary is up and serving real RPCs. A TCP
   * connect proves the gRPC service has bound to the port, which is what
   * we actually want to know.
   */
  private probe(): Promise<boolean> {
    return new Promise((resolve) => {
      const socket = connect(this.opts.port, "127.0.0.1");
      const timer = setTimeout(() => {
        socket.destroy();
        resolve(false);
      }, 2_000);
      socket.once("connect", () => {
        clearTimeout(timer);
        socket.end();
        resolve(true);
      });
      socket.once("error", () => {
        clearTimeout(timer);
        resolve(false);
      });
    });
  }

  async stop(): Promise<void> {
    const child = this.child;
    if (!child) return;
    try {
      child.kill("SIGTERM");
    } catch {
      /* ignore */
    }
    await new Promise<void>((resolve) => {
      const timer = setTimeout(() => {
        try {
          child.kill("SIGKILL");
        } catch {
          /* ignore */
        }
        resolve();
      }, STOP_GRACE_MS);
      child.once("exit", () => {
        clearTimeout(timer);
        resolve();
      });
    });
    this.child = null;
  }
}

export interface GrpcCommandResult {
  command: string;
  ok: boolean;
  exitCode: number | null;
  stdout: string;
  stderr: string;
  durationMs: number;
  timedOut: boolean;
}

/**
 * Run a single grpcurl command (or any bash one-liner) and capture the
 * result. We delegate to `bash -c` so the user's JSON payloads (which
 * usually live inside single quotes) survive intact without a shell-quote
 * dep.
 */
export async function runGrpcCommand(
  command: string,
  cwd: string,
  timeoutMs = 30_000
): Promise<GrpcCommandResult> {
  const started = Date.now();
  const result = await execa("bash", ["-c", command], {
    cwd,
    reject: false,
    timeout: timeoutMs,
    all: true,
  });
  const durationMs = Date.now() - started;
  const timedOut = result.timedOut === true;
  return {
    command,
    ok: result.exitCode === 0 && !timedOut,
    exitCode: result.exitCode ?? null,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
    durationMs,
    timedOut,
  };
}

/** Quick preflight: is `grpcurl` even installed on this host? */
export async function isGrpcurlInstalled(): Promise<boolean> {
  try {
    const result = await execa("grpcurl", ["--version"], {
      reject: false,
      timeout: 5_000,
    });
    return result.exitCode === 0;
  } catch {
    return false;
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}
