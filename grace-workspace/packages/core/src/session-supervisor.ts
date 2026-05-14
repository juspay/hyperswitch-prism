import { type ChildProcess, spawn } from "node:child_process";
import { WebSocketServer, WebSocket } from "ws";

import { newRunId } from "./utils.js";
import {
  DEFAULT_SESSION_ID,
  type SessionRecord,
  type StateManager,
} from "./state.js";
import type { SessionManager } from "./session-manager.js";

/**
 * Cadence at which the supervisor reaps zombie children (PID gone) and
 * runs the heartbeat-based stale-lock cleanup. 10s keeps reaction time
 * snappy without flooding the DB.
 */
const REAP_INTERVAL_MS = 10_000;

/** Per-engine stale-heartbeat threshold inside the running supervisor. */
const STALE_HEARTBEAT_MS = 60_000;

/** Soft-kill grace period before SIGKILL escalation. */
const TERM_GRACE_MS = 5_000;

/** Per-session replay buffer cap. Big enough for a full pipeline replay. */
const REPLAY_BUFFER_LIMIT = 500;

/**
 * Inbound messages prefixed with this string are handled by the supervisor
 * itself (session CRUD, lifecycle). Anything else is forwarded to the
 * engine for the dashboard's subscribed session.
 */
const CONTROL_PREFIXES = ["sessions:"] as const;

interface ActiveChild {
  sessionId: string;
  runId: string;
  pid: number;
  child: ChildProcess;
  startedAt: number;
}

interface DashboardClient {
  ws: WebSocket;
  /** Set when the dashboard sends `hello {role:'dashboard', sessionId}`.
   * `undefined` = Homepage-style dashboard, only receives `sessions:*`
   * broadcasts. */
  sessionId?: string;
}

/**
 * Optional intent for `startSession` that lets the caller:
 *  - re-use an existing run row (resume): pass `runId`
 *  - request the resumed run start at a specific checkpoint: pass `startFrom`
 *
 * Captured automatically from a child engine's `__TENXGRACE_RESPAWN__` stdout
 * marker so a `runs:resume` from the dashboard cleanly transitions to a
 * fresh child engine pointed at the same run.
 */
export interface StartIntent {
  runId?: string;
  startFrom?: string;
}

export interface SupervisorOptions {
  /** Absolute path to the CLI entry (packages/cli/dist/index.js). */
  cliEntryPath: string;
  /** Path forwarded to children as --config. Required for non-default config. */
  configPath?: string;
  /** Forward NODE_ENV / extra vars to children. Defaults to process.env. */
  env?: NodeJS.ProcessEnv;
}

interface InboundMsg {
  type: string;
  payload?: Record<string, unknown>;
}

/**
 * SessionSupervisor — owns the lifecycle of per-session engine child
 * processes AND the single multiplexed control WebSocket.
 *
 * Phase 5 architecture: the supervisor is the *only* WebSocket server in
 * the system. It listens on `controlWsPort` (cfg.wsPort, e.g. 3334) and
 * accepts two kinds of clients, distinguished by the `hello` frame each
 * one sends as its first message:
 *
 *   - `{role:'dashboard', sessionId?}` — a browser tab. Without sessionId,
 *     it sees only `sessions:*` broadcasts (Homepage). With sessionId, it
 *     sees that session's pipeline events and inbound messages route to
 *     that session's engine.
 *
 *   - `{role:'engine', sessionId, runId}` — a child engine process. The
 *     supervisor stashes this connection in `engineSockets[sessionId]` and
 *     relays its outbound events to subscribed dashboards.
 *
 * Engine children no longer listen on their own ports. They are *clients*
 * to this server. That removes the per-session port pool entirely and
 * lets the dashboard use one URL.
 *
 * Liveness has two layers:
 * 1. PID liveness via `process.kill(pid, 0)` — catches `kill -9` and
 *    parent-orphaned children immediately on the next reap tick.
 * 2. Heartbeat-based via {@link StateManager.recoverStaleSessions} — catches
 *    children whose process is alive but stuck (no checkpoint progress).
 */
export class SessionSupervisor {
  private wss: WebSocketServer;
  private dashboards = new Set<DashboardClient>();
  private engineSockets = new Map<string, WebSocket>();
  private replayBuffers = new Map<string, string[]>();
  private active = new Map<string, ActiveChild>();
  private reapTimer: NodeJS.Timeout | null = null;
  private shuttingDown = false;

  constructor(
    private state: StateManager,
    private sessions: SessionManager,
    private controlWsPort: number,
    private opts: SupervisorOptions
  ) {
    this.wss = new WebSocketServer({ port: controlWsPort });
    this.wss.on("connection", (ws) => this.onConnection(ws));
    this.wss.on("error", (err) => {
      // eslint-disable-next-line no-console
      console.error(`[supervisor] WS error:`, err);
    });

    this.recoverFromCrash();
    this.reapTimer = setInterval(() => this.reapTick(), REAP_INTERVAL_MS);
    this.reapTimer.unref?.();

    process.on("SIGTERM", () => void this.shutdown("SIGTERM"));
    process.on("SIGINT", () => void this.shutdown("SIGINT"));
  }

  /**
   * Boot-time recovery: any session row whose `pid` is set must be checked
   * — if the pid is gone (we just restarted the supervisor), wipe its
   * runtime fields and let `recoverStaleSessions` flip its run to failed.
   */
  private recoverFromCrash(): void {
    for (const s of this.state.listSessions()) {
      if (s.pid !== null && !this.isAlive(s.pid)) {
        // eslint-disable-next-line no-console
        console.log(
          `[supervisor] reaping crashed session ${s.sessionId} (pid=${s.pid} no longer alive)`
        );
        this.state.updateSessionRuntime(s.sessionId, { wsPort: null, pid: null });
      } else if (s.pid !== null) {
        // pid is alive AND in DB — orphaned from a prior supervisor we don't
        // own. Safer to kill and reclaim than risk double-scheduling.
        // eslint-disable-next-line no-console
        console.log(
          `[supervisor] orphan engine pid=${s.pid} for session ${s.sessionId}; killing`
        );
        try {
          process.kill(s.pid, "SIGTERM");
        } catch {
          /* ignore */
        }
        this.state.updateSessionRuntime(s.sessionId, { wsPort: null, pid: null });
      }
    }
    const cleared = this.state.recoverStaleSessions(5_000);
    if (cleared > 0) {
      // eslint-disable-next-line no-console
      console.log(
        `[supervisor] recovered ${cleared} stale session lock(s) from prior boot`
      );
    }
  }

  private isAlive(pid: number): boolean {
    try {
      process.kill(pid, 0);
      return true;
    } catch {
      return false;
    }
  }

  // ─── WebSocket dispatch ────────────────────────────────────────────────

  private onConnection(ws: WebSocket): void {
    // The first message MUST be a hello frame so we know whether this is a
    // dashboard or an engine. Until we get it, the connection is in limbo.
    let helloHandled = false;
    const onMessage = (raw: WebSocket.RawData) => {
      let msg: InboundMsg;
      try {
        msg = JSON.parse(raw.toString()) as InboundMsg;
      } catch {
        return;
      }
      if (!helloHandled) {
        if (msg.type !== "hello") {
          // eslint-disable-next-line no-console
          console.warn(`[supervisor] first frame was ${msg.type}, expected 'hello' — closing`);
          ws.close();
          return;
        }
        helloHandled = true;
        const payload = (msg.payload ?? {}) as {
          role?: string;
          sessionId?: string;
          runId?: string;
        };
        if (payload.role === "engine") {
          if (!payload.sessionId || !payload.runId) {
            // eslint-disable-next-line no-console
            console.warn(
              `[supervisor] engine hello missing sessionId/runId — closing`
            );
            ws.close();
            return;
          }
          this.attachEngine(ws, {
            sessionId: payload.sessionId,
            runId: payload.runId,
          });
        } else {
          this.attachDashboard(ws, { sessionId: payload.sessionId });
        }
        return;
      }
      // Subsequent messages routed by role (the role-specific handlers
      // were registered when we attached).
    };
    ws.on("message", onMessage);
    ws.on("close", () => {
      // close handlers added by attach* take care of cleanup; this is a
      // safety net for sockets that disconnected before sending hello.
      if (!helloHandled) {
        // eslint-disable-next-line no-console
        console.log(`[supervisor] socket closed before hello`);
      }
    });
    ws.on("error", (err) => {
      // eslint-disable-next-line no-console
      console.error(`[supervisor] socket error:`, err);
    });
  }

  // ─── Dashboard side ────────────────────────────────────────────────────

  private attachDashboard(
    ws: WebSocket,
    hello: { sessionId?: string }
  ): void {
    const dc: DashboardClient = { ws, sessionId: hello.sessionId };
    this.dashboards.add(dc);

    // Greet the new dashboard with the current session list so the
    // Homepage renders without a round-trip. Engine-replay buffer is sent
    // separately if the dashboard is session-subscribed.
    this.sendRaw(ws, JSON.stringify({
      type: "sessions:snapshot",
      payload: { sessions: this.state.listSessions() },
    }));

    if (dc.sessionId) {
      const buf = this.replayBuffers.get(dc.sessionId);
      if (buf) {
        for (const frame of buf) this.sendRaw(ws, frame);
      }
    }

    // Replace the bootstrap onMessage that just consumed `hello`.
    ws.removeAllListeners("message");
    ws.on("message", (raw) => {
      let msg: InboundMsg;
      try {
        msg = JSON.parse(raw.toString()) as InboundMsg;
      } catch {
        return;
      }
      void this.handleDashboardMessage(dc, msg);
    });
    ws.on("close", () => this.dashboards.delete(dc));
    // eslint-disable-next-line no-console
    console.log(
      `[supervisor] dashboard connected${dc.sessionId ? ` (subscribed to ${dc.sessionId})` : ""}`
    );
  }

  private async handleDashboardMessage(
    dc: DashboardClient,
    msg: InboundMsg
  ): Promise<void> {
    if (this.isControlMessage(msg.type)) {
      await this.handleControl(dc.ws, msg);
      return;
    }
    // Pipeline-bound: relay to engine for the dashboard's subscribed session.
    if (!dc.sessionId) {
      // eslint-disable-next-line no-console
      console.warn(
        `[supervisor] dashboard sent ${msg.type} without subscribing to a session — dropping`
      );
      return;
    }
    const eng = this.engineSockets.get(dc.sessionId);
    if (!eng || eng.readyState !== eng.OPEN) {
      // eslint-disable-next-line no-console
      console.warn(
        `[supervisor] no live engine for ${dc.sessionId} to receive ${msg.type}`
      );
      return;
    }
    try {
      eng.send(JSON.stringify(msg));
    } catch (err) {
      // eslint-disable-next-line no-console
      console.error(`[supervisor] relay to engine failed:`, err);
    }
  }

  private isControlMessage(type: string): boolean {
    return CONTROL_PREFIXES.some((p) => type.startsWith(p));
  }

  // ─── Engine side ───────────────────────────────────────────────────────

  private attachEngine(
    ws: WebSocket,
    hello: { sessionId: string; runId: string }
  ): void {
    const sessionId = hello.sessionId;
    if (!sessionId) {
      // eslint-disable-next-line no-console
      console.warn(`[supervisor] engine hello missing sessionId — closing`);
      ws.close();
      return;
    }

    // If a stale engine connection exists for this session, close it.
    const existing = this.engineSockets.get(sessionId);
    if (existing && existing !== ws) {
      try { existing.close(); } catch { /* ignore */ }
    }
    this.engineSockets.set(sessionId, ws);
    // Per-session replay buffer; clear when a new engine attaches so the
    // dashboard sees a clean event stream for the fresh run.
    this.replayBuffers.set(sessionId, []);

    ws.removeAllListeners("message");
    ws.on("message", (raw) => {
      const text = raw.toString();
      // We don't need to parse to relay; just forward bytes. Buffer for
      // late-joining dashboards.
      const buf = this.replayBuffers.get(sessionId);
      if (buf) {
        buf.push(text);
        if (buf.length > REPLAY_BUFFER_LIMIT) buf.shift();
      }
      this.broadcastToSession(sessionId, text);
    });
    ws.on("close", () => {
      if (this.engineSockets.get(sessionId) === ws) {
        this.engineSockets.delete(sessionId);
      }
    });

    // eslint-disable-next-line no-console
    console.log(
      `[supervisor] engine attached: session=${sessionId} runId=${hello.runId}`
    );
  }

  private broadcastToSession(sessionId: string, frame: string): void {
    for (const dc of this.dashboards) {
      if (dc.sessionId === sessionId && dc.ws.readyState === dc.ws.OPEN) {
        try { dc.ws.send(frame); } catch { /* ignore */ }
      }
    }
  }

  /** Broadcast a control message (sessions:*) to ALL connected dashboards. */
  private broadcastControl(type: string, payload: unknown): void {
    const data = JSON.stringify({ type, payload });
    for (const dc of this.dashboards) {
      if (dc.ws.readyState === dc.ws.OPEN) {
        try { dc.ws.send(data); } catch { /* ignore */ }
      }
    }
  }

  private sendRaw(ws: WebSocket, frame: string): void {
    if (ws.readyState !== ws.OPEN) return;
    try { ws.send(frame); } catch { /* ignore */ }
  }

  private send(ws: WebSocket, type: string, payload: unknown): void {
    this.sendRaw(ws, JSON.stringify({ type, payload }));
  }

  // ─── Control message handlers ──────────────────────────────────────────

  private async handleControl(ws: WebSocket, msg: InboundMsg): Promise<void> {
    const payload = (msg.payload ?? {}) as Record<string, unknown>;
    switch (msg.type) {
      case "sessions:list": {
        this.send(ws, "sessions:list:response", {
          sessions: this.state.listSessions(),
        });
        return;
      }
      case "sessions:create": {
        try {
          // DEBUG: Log what we receive
          console.log("[SUPERVISOR] Received sessions:create payload:", {
            hasInitialTask: !!payload.initialTask,
            initialTaskKeys: payload.initialTask ? Object.keys(payload.initialTask as object) : [],
            runner: (payload.initialTask as Record<string, unknown>)?.runner,
            runnerModel: (payload.initialTask as Record<string, unknown>)?.runnerModel,
            fullPayload: JSON.stringify(payload).slice(0, 500),
          });
          
          const session = await this.sessions.create({
            name: String(payload.name ?? ""),
            description: payload.description as string | undefined,
            sourcePath: String(payload.sourcePath ?? ""),
            strategy:
              (payload.strategy as "git-worktree" | "full" | "shallow") ??
              "git-worktree",
            initialTask: payload.initialTask as import("./types.js").TaskDefinition | undefined,
          });
          this.broadcastControl("sessions:created", { session });
        } catch (err) {
          this.send(ws, "sessions:create:error", {
            error: err instanceof Error ? err.message : String(err),
          });
        }
        return;
      }
      case "sessions:start": {
        const sessionId = String(payload.sessionId ?? "");
        try {
          const ac = await this.startSession(sessionId);
          this.send(ws, "sessions:start:response", {
            sessionId: ac.sessionId,
            runId: ac.runId,
          });
        } catch (err) {
          this.send(ws, "sessions:start:error", {
            sessionId,
            error: err instanceof Error ? err.message : String(err),
          });
        }
        return;
      }
      case "sessions:stop": {
        await this.stopSession(String(payload.sessionId ?? ""));
        return;
      }
      case "sessions:archive": {
        const sessionId = String(payload.sessionId ?? "");
        try {
          await this.sessions.archive(sessionId);
          this.broadcastControl("sessions:archived", { sessionId });
        } catch (err) {
          this.send(ws, "sessions:archive:error", {
            sessionId,
            error: err instanceof Error ? err.message : String(err),
          });
        }
        return;
      }
      case "sessions:delete": {
        const sessionId = String(payload.sessionId ?? "");
        try {
          if (this.active.has(sessionId)) await this.stopSession(sessionId);
          await this.sessions.delete(sessionId);
          this.broadcastControl("sessions:deleted", { sessionId });
        } catch (err) {
          this.send(ws, "sessions:delete:error", {
            sessionId,
            error: err instanceof Error ? err.message : String(err),
          });
        }
        return;
      }
      default:
        return;
    }
  }

  // ─── Child lifecycle ───────────────────────────────────────────────────

  /**
   * Idempotent: if the session already has a live child, return it. Otherwise
   * enqueue a pending run (unless `intent.runId` is given, in which case we
   * re-use that existing run row), spawn `node cli/dist/index.js run
   * --session … --resume <runId>` (with --start-from when supplied), and
   * register the child.
   *
   * The spawned child connects back to the supervisor's control WS as an
   * engine client; events flow through that bidirectional pipe.
   *
   * The spawned child's stdout is line-parsed for `__TENXGRACE_RESPAWN__ <json>`
   * markers; if one is seen, on the child's next exit we automatically call
   * `startSession(sessionId, capturedIntent)` to roll into a fresh engine.
   */
  async startSession(sessionId: string, intent?: StartIntent): Promise<ActiveChild> {
    const existing = this.active.get(sessionId);
    if (existing && this.isAlive(existing.pid)) return existing;
    if (existing) {
      this.onChildExit(sessionId, -1, "stale-pre-start");
    }

    const session = this.state.getSession(sessionId);
    if (!session) throw new Error(`No such session: ${sessionId}`);
    if (session.status === "archived") {
      throw new Error(`Session ${sessionId} is archived`);
    }

    let runId: string;
    // Track if we had initialTask (for deciding whether to skip task checkpoint)
    let hasInitialTask = false;
    
    if (intent?.runId) {
      runId = intent.runId;
    } else {
      runId = newRunId();
      // Check for initialTask in session metadata (from unified create modal)
      const initialTask = session.metadata?.initialTask;
      // Capture this BEFORE we clear the metadata below
      hasInitialTask = !!initialTask;
      
      const task = initialTask
        ? {
            title: initialTask.title,
            description: initialTask.description,
            acceptanceCriteria: initialTask.acceptanceCriteria,
            projectRoot: session.projectRoot,
            sessionId,
            paymentMethod: initialTask.paymentMethod,
            targetConnectors: initialTask.targetConnectors,
            paymentMethodCategory: (initialTask as unknown as { category?: string }).category,
            priority: initialTask.priority,
            runner: initialTask.runner,
            runnerModel: initialTask.runnerModel,
            connectorDocUrls: [],
          }
        : {
            title: "",
            description: "",
            acceptanceCriteria: [] as string[],
            projectRoot: session.projectRoot,
            sessionId,
          };
      this.state.enqueueRun(sessionId, runId, task);
      
      // Clear initialTask after using it so it doesn't run twice
      if (initialTask) {
        this.state.updateSessionMetadata(sessionId, {
          ...session.metadata,
          initialTask: undefined,
        });
      }
    }
    
    const args = [
      this.opts.cliEntryPath,
      "run",
      "--session",
      sessionId,
      "--resume",
      runId,
    ];
    
    // Only use --task-from-ui when we DON'T have an initial task
    // When we have initialTask, we skip task checkpoint and go to preflight
    if (!hasInitialTask) {
      args.push("--task-from-ui");
    }
    
    // If we have initialTask and no explicit startFrom, start from preflight (skip task checkpoint)
    if (hasInitialTask && !intent?.startFrom) {
      args.push("--start-from", "preflight");
    } else if (intent?.startFrom) {
      args.push("--start-from", intent.startFrom);
    }
    
    if (this.opts.configPath) args.push("--config", this.opts.configPath);

    const child = spawn(process.execPath, args, {
      stdio: ["ignore", "pipe", "pipe"],
      env: this.opts.env ?? process.env,
      detached: false,
    });

    if (!child.pid) {
      throw new Error(`Failed to spawn engine child for session ${sessionId}`);
    }

    const tag = `\x1b[36m[${sessionId.slice(0, 14)}]\x1b[0m`;

    let pendingRespawn: StartIntent | null = null;
    let stdoutBuffer = "";
    child.stdout?.on("data", (buf: Buffer) => {
      process.stdout.write(prefixLines(tag, buf));
      stdoutBuffer += buf.toString("utf8");
      let nl: number;
      while ((nl = stdoutBuffer.indexOf("\n")) >= 0) {
        const line = stdoutBuffer.slice(0, nl);
        stdoutBuffer = stdoutBuffer.slice(nl + 1);
        const m = line.match(/__TENXGRACE_RESPAWN__\s+(\{[^}]*\})/);
        if (m) {
          try {
            pendingRespawn = JSON.parse(m[1]) as StartIntent;
            // eslint-disable-next-line no-console
            console.log(
              `[supervisor] respawn intent for ${sessionId}: ${JSON.stringify(pendingRespawn)}`
            );
          } catch (err) {
            // eslint-disable-next-line no-console
            console.error(`[supervisor] bad respawn marker:`, err);
          }
        }
      }
    });
    child.stderr?.on("data", (buf: Buffer) =>
      process.stderr.write(prefixLines(tag, buf))
    );

    const ac: ActiveChild = {
      sessionId,
      runId,
      pid: child.pid,
      child,
      startedAt: Date.now(),
    };
    this.active.set(sessionId, ac);
    // Multiplex architecture: ws_port no longer applies. Keep pid for liveness.
    this.state.updateSessionRuntime(sessionId, { wsPort: null, pid: child.pid });

    child.on("exit", (code) => {
      this.onChildExit(sessionId, code, "exit");
      if (pendingRespawn && !this.shuttingDown) {
        const intentToUse = pendingRespawn;
        setTimeout(() => {
          this.startSession(sessionId, intentToUse).catch((err) => {
            // eslint-disable-next-line no-console
            console.error(`[supervisor] respawn failed for ${sessionId}:`, err);
            this.broadcastControl("sessions:respawn:error", {
              sessionId,
              error: err instanceof Error ? err.message : String(err),
            });
          });
        }, 100);
      }
    });
    child.on("error", (err) => {
      // eslint-disable-next-line no-console
      console.error(`[supervisor] child error for ${sessionId}:`, err);
    });

    // eslint-disable-next-line no-console
    console.log(
      `[supervisor] started session=${sessionId} runId=${runId} pid=${child.pid}${intent?.startFrom ? ` startFrom=${intent.startFrom}` : ""}`
    );
    this.broadcastControl("sessions:started", {
      sessionId,
      runId,
      pid: child.pid,
    });
    return ac;
  }

  async stopSession(sessionId: string): Promise<void> {
    const ac = this.active.get(sessionId);
    if (!ac) return;
    try {
      this.state.releaseSession(sessionId, ac.runId, "cancelled");
    } catch {
      /* best-effort */
    }
    try {
      ac.child.kill("SIGTERM");
    } catch {
      /* ignore */
    }
    setTimeout(() => {
      if (this.active.has(sessionId)) {
        try {
          ac.child.kill("SIGKILL");
        } catch {
          /* ignore */
        }
      }
    }, TERM_GRACE_MS).unref?.();
  }

  private onChildExit(
    sessionId: string,
    code: number | null,
    reason: string
  ): void {
    const ac = this.active.get(sessionId);
    if (!ac) return;
    this.active.delete(sessionId);
    this.state.updateSessionRuntime(sessionId, { wsPort: null, pid: null });
    // Tear down the engine WS socket if it lingered.
    const sock = this.engineSockets.get(sessionId);
    if (sock) {
      try { sock.close(); } catch { /* ignore */ }
      this.engineSockets.delete(sessionId);
    }
    // Intentionally do NOT delete this.replayBuffers here. The buffer holds
    // the just-completed (or just-failed) run's events — preserving it means
    // a dashboard tab that visits the session after the engine has exited
    // can still replay the post-mortem state. attachEngine clears the buffer
    // on the *next* engine connect, which is the correct moment to reset.
    // eslint-disable-next-line no-console
    console.log(
      `[supervisor] session=${sessionId} exited code=${code ?? "null"} (${reason})`
    );
    this.broadcastControl("sessions:exited", { sessionId, code });

    // Phase 10: revert per-session config rewrites that preflight did at
    // run start (currently <projectRoot>/config/development.toml's
    // dummyconnector.base_url). Fire-and-forget — failures are logged but
    // don't block the exit path.
    void this.sessions.restoreSessionConfigs(sessionId).catch((err) => {
      // eslint-disable-next-line no-console
      console.error(`[supervisor] restoreSessionConfigs failed:`, err);
    });
  }

  private reapTick(): void {
    if (this.shuttingDown) return;
    for (const [sessionId, ac] of this.active) {
      if (!this.isAlive(ac.pid)) {
        this.onChildExit(sessionId, -1, "pid-vanished");
      }
    }
    try {
      const cleared = this.state.recoverStaleSessions(STALE_HEARTBEAT_MS);
      if (cleared > 0) {
        // eslint-disable-next-line no-console
        console.log(`[supervisor] reaped ${cleared} stale lock(s) by heartbeat`);
      }
    } catch (err) {
      // eslint-disable-next-line no-console
      console.error(`[supervisor] reap error:`, err);
    }
  }

  async shutdown(reason: string): Promise<void> {
    if (this.shuttingDown) return;
    this.shuttingDown = true;
    // eslint-disable-next-line no-console
    console.log(`[supervisor] shutting down (${reason}) — ${this.active.size} active session(s)`);
    if (this.reapTimer) clearInterval(this.reapTimer);

    for (const ac of this.active.values()) {
      try { ac.child.kill("SIGTERM"); } catch { /* ignore */ }
    }
    await sleep(TERM_GRACE_MS);
    for (const ac of this.active.values()) {
      try { ac.child.kill("SIGKILL"); } catch { /* ignore */ }
    }

    this.wss.close();
    this.state.close();
    process.exit(0);
  }

  /** For tests / introspection. */
  listActive(): ReadonlyMap<string, ActiveChild> {
    return this.active;
  }
}

function prefixLines(tag: string, buf: Buffer): string {
  const text = buf.toString("utf8");
  return text
    .split("\n")
    .map((ln, i, arr) =>
      i === arr.length - 1 && ln === "" ? "" : `${tag} ${ln}\n`
    )
    .join("");
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

/** @internal — for tests. */
export type { ActiveChild };

/** Snapshot the supervisor projects out for SessionRecord-shaped consumers. */
export function sessionRuntimeSummary(s: SessionRecord) {
  return {
    sessionId: s.sessionId,
    pid: s.pid,
    status: s.status,
  };
}
