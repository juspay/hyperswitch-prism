import { type ChildProcess, spawn } from "node:child_process";
import { execa } from "execa";

/**
 * Spawns `cargo run -p grpc-server` in the worktree and waits for the
 * server to answer the gRPC reflection list before returning. Used for
 * the gRPC verification step that runs after build/clippy pass.
 *
 * MVP scope: one server per test step. Started and torn down for every
 * resolve cycle. Future work: keep it warm across retries.
 */

const HEALTH_TIMEOUT_MS = 120_000;
const HEALTH_POLL_MS = 2_000;
const STOP_GRACE_MS = 5_000;
const DEFAULT_CARGO_BIN = "cargo";

export interface GrpcServerOptions {
  worktreePath: string;
  port: number;
  cargoBin?: string;
  /** Extra env passed through, e.g. RUST_LOG, BYNE creds. */
  env?: NodeJS.ProcessEnv;
  /** Called with each line of server stderr for log streaming. */
  onStderr?: (line: string) => void;
}

export class GrpcServerProcess {
  private child: ChildProcess | null = null;
  private exited = false;
  private exitReason: string | null = null;

  constructor(private readonly opts: GrpcServerOptions) {}

  async start(): Promise<void> {
    if (this.child) {
      throw new Error("GrpcServerProcess.start: already running");
    }
    const args = ["run", "-p", "grpc-server"];
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
    const deadline = Date.now() + HEALTH_TIMEOUT_MS;
    while (Date.now() < deadline) {
      if (this.exited) {
        throw new Error(
          `grpc-server exited before becoming healthy (${this.exitReason ?? "?"}).`
        );
      }
      const ok = await this.probe();
      if (ok) return;
      await sleep(HEALTH_POLL_MS);
    }
    await this.stop().catch(() => undefined);
    throw new Error(
      `grpc-server didn't become healthy within ${HEALTH_TIMEOUT_MS}ms (port ${this.opts.port}).`
    );
  }

  private async probe(): Promise<boolean> {
    try {
      const result = await execa(
        "grpcurl",
        ["-plaintext", `localhost:${this.opts.port}`, "list"],
        { reject: false, timeout: 5_000 }
      );
      return result.exitCode === 0;
    } catch {
      return false;
    }
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
