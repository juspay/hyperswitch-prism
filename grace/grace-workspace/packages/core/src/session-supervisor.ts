import { type ChildProcess, spawn } from "node:child_process";
import { WebSocketServer, WebSocket } from "ws";

import { newRunId } from "./utils.js";
import {
  DEFAULT_SESSION_ID,
  type SessionRecord,
  type StateManager,
} from "./state.js";
import type { SessionManager } from "./session-manager.js";
import { getConfig, type PrResolverConfig } from "./config.js";
import {
  PrResolverService,
  getRecentPrResolverEvents,
  isReplayablePrResolverEvent,
  loadRuntimeOverlay,
  mergeWithOverlay,
  onPrResolverEvent,
  saveRuntimeOverlay,
  validateOverlay,
  type PrResolverRuntimeOverlay,
} from "./pr-resolver/index.js";

/**
 * Cadence at which the supervisor reaps zombie children (PID gone) and
 * runs the heartbeat-based stale-lock cleanup. 10s keeps reaction time
 * snappy without flooding the DB.
 */
const REAP_INTERVAL_MS = 10_000;

/** Per-engine stale-heartbeat threshold inside the running supervisor. */
const STALE_HEARTBEAT_MS = 60_000;

/**
 * Boot-time reap threshold. Any session lock with a heartbeat older than this
 * at supervisor startup is treated as a leftover from a crashed prior boot.
 * Aggressive (5s) by design — we only run this once at startup, so a slow but
 * genuinely-alive engine from a *prior* supervisor can't race here.
 */
const BOOT_STALE_THRESHOLD_MS = 5_000;

/** Soft-kill grace period before SIGKILL escalation. */
const TERM_GRACE_MS = 5_000;

/** Per-session replay buffer cap. Big enough for a full pipeline replay. */
const REPLAY_BUFFER_LIMIT = 500;

/**
 * High-frequency PR Resolver events (one per line of Claude / cargo stdout)
 * are kept out of the main 500-entry replay buffer to avoid evicting
 * state-shaping events. Instead we maintain per-PR rolling tails of the last
 * N lines for `resolver_stream` and `grpc_server_log` and ship them inside
 * every `pr-resolver:snapshot` so a dashboard refresh / reconnect / late join
 * still shows the most recent live output.
 */
const PR_STREAM_TAIL_LIMIT = 200;

/**
 * Inbound messages prefixed with these strings are handled by the supervisor
 * itself (session CRUD, lifecycle, PR-resolver controls). Anything else is
 * forwarded to the engine for the dashboard's subscribed session.
 */
const CONTROL_PREFIXES = ["sessions:", "pr-resolver:"] as const;

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
  /** Lazily booted in `bootPrResolver` when the effective config has enabled=true. */
  private prResolver: PrResolverService | null = null;
  private prResolverUnsub: (() => void) | null = null;
  private prResolverTask: Promise<void> | null = null;
  /** User-set runtime overlay loaded from `~/.tenxgrace/pr-resolver-config.json` (or legacy `~/.byne/...`). */
  private prResolverOverlay: PrResolverRuntimeOverlay = {};
  /** Merged config currently driving the running service. */
  private prResolverEffective: PrResolverConfig | null = null;
  /** Per-PR rolling tail of `pr-resolver:resolver_stream` lines. */
  private prResolverStreamTails = new Map<number, string[]>();
  /** Per-PR rolling tail of `pr-resolver:grpc_server_log` lines. */
  private prGrpcServerLogTails = new Map<number, string[]>();

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

    // PR Resolver — opt-in via prResolver.enabled. Fire-and-forget boot so a
    // slow `gh repo clone` on first run doesn't block the WS server from
    // accepting dashboards.
    void this.bootPrResolver();
  }

  // ─── PR Resolver lifecycle ─────────────────────────────────────────────

  private getBasePrResolverConfig(): PrResolverConfig | null {
    try {
      return getConfig().prResolver;
    } catch {
      return null;
    }
  }

  private getEffectivePrResolverConfig(): PrResolverConfig | null {
    const base = this.getBasePrResolverConfig();
    if (!base) return null;
    return mergeWithOverlay(base, this.prResolverOverlay);
  }

  /**
   * Boot-time entry point: load the runtime overlay, then start the service
   * if the merged config has enabled=true and githubRepo set. Safe to call
   * once at construction; later mutations go through `reconfigurePrResolver`.
   */
  private async bootPrResolver(): Promise<void> {
    if (!this.getBasePrResolverConfig()) return;
    this.prResolverOverlay = loadRuntimeOverlay();
    await this.startPrResolverIfEnabled();
  }

  private async startPrResolverIfEnabled(): Promise<void> {
    const effective = this.getEffectivePrResolverConfig();
    if (!effective) return;
    this.prResolverEffective = effective;

    if (!effective.enabled) {
      // eslint-disable-next-line no-console
      console.log(
        `\x1b[90m[supervisor] PR Resolver disabled (config.yml + runtime overlay).\x1b[0m`
      );
      return;
    }
    if (!effective.githubRepo) {
      // eslint-disable-next-line no-console
      console.warn(
        `[supervisor] prResolver.enabled is true but githubRepo is empty — skipping boot`
      );
      return;
    }

    try {
      this.prResolver = new PrResolverService(effective, getConfig());
    } catch (err) {
      // eslint-disable-next-line no-console
      console.error(
        `[supervisor] PR Resolver init failed:`,
        err instanceof Error ? err.message : err
      );
      return;
    }

    // Broadcast every event from the resolver to all dashboards. The bus
    // also keeps a 500-entry replay buffer so late-joining dashboards catch
    // up via the `pr-resolver:snapshot` we send on hello. High-frequency
    // events (`resolver_stream`, `grpc_server_log`) bypass that buffer but
    // are tee'd into per-PR rolling tails so the snapshot can still ship
    // recent lines to reconnecting dashboards.
    this.prResolverUnsub = onPrResolverEvent((event) => {
      // Reset per-PR rolling tails at the start of a fresh attempt so a
      // retry / re-poll doesn't keep streaming the prior cycle's final
      // lines (and seeding them into every snapshot).
      this.resetPrStreamTailIfBoundary(event);
      this.appendPrStreamTail(event);
      this.broadcastControl(`pr-resolver:${event.type}`, {
        ...event.payload,
        timestamp: event.timestamp,
      });
      // Any state-mutating event (everything except the high-volume stream
      // tails) is a hint that PrMachine state may have changed server-side.
      // The dashboard's `prMachines` is only refreshed via snapshot, so we
      // debounce-broadcast one here. Without this, the approval gate, retry
      // banner, and verification stage all stay stale until the user clicks
      // something or refreshes the page.
      if (isReplayablePrResolverEvent(event.type)) {
        this.scheduleSnapshotBroadcast();
      }
    });

    // eslint-disable-next-line no-console
    console.log(
      `\x1b[1m\x1b[35m[supervisor]\x1b[0m PR Resolver enabled · repo=${effective.githubRepo} · trigger="${effective.trigger}" · interval=${effective.pollInterval}s`
    );

    this.prResolverTask = this.prResolver
      .runForever()
      .catch((err) => {
        // eslint-disable-next-line no-console
        console.error(
          `[supervisor] PR Resolver crashed:`,
          err instanceof Error ? err.stack ?? err.message : err
        );
        this.broadcastControl("pr-resolver:crash", {
          error: err instanceof Error ? err.message : String(err),
        });
      });
  }

  private async stopPrResolver(): Promise<void> {
    if (!this.prResolver) return;
    this.prResolver.cancel();
    this.prResolverUnsub?.();
    this.prResolverUnsub = null;
    if (this.prResolverSnapshotTimer) {
      clearTimeout(this.prResolverSnapshotTimer);
      this.prResolverSnapshotTimer = null;
    }
    try {
      await Promise.race([this.prResolverTask, sleep(TERM_GRACE_MS)]);
    } catch {
      /* best-effort drain */
    }
    this.prResolver = null;
    this.prResolverTask = null;
  }

  /**
   * Apply a new runtime overlay: validate, persist, stop the running service,
   * start it again with the merged config. Errors short-circuit before any
   * side effect.
   */
  private async reconfigurePrResolver(
    newOverlay: PrResolverRuntimeOverlay
  ): Promise<{ ok: boolean; errors?: string[] }> {
    const validation = validateOverlay(newOverlay);
    if (!validation.ok) {
      return { ok: false, errors: validation.errors };
    }
    // Persist before stopping so a crash mid-restart still picks up the
    // intended config on the next supervisor boot.
    try {
      saveRuntimeOverlay(newOverlay);
    } catch (err) {
      return {
        ok: false,
        errors: [
          `Failed to write overlay: ${err instanceof Error ? err.message : String(err)}`,
        ],
      };
    }
    this.prResolverOverlay = newOverlay;
    await this.stopPrResolver();
    await this.startPrResolverIfEnabled();
    this.broadcastPrResolverSnapshot();
    return { ok: true };
  }

  /** Build the per-dashboard snapshot payload. Shared by hello + broadcast. */
  private buildPrResolverSnapshotPayload(): Record<string, unknown> {
    const effective =
      this.prResolverEffective ?? this.getEffectivePrResolverConfig();
    const stateSnap = this.prResolver?.getStateSnapshot() ?? null;
    return {
      enabled: !!effective?.enabled,
      autoApprove: !!effective?.autoApprove,
      githubRepo: effective?.githubRepo ?? "",
      trigger: effective?.trigger ?? "",
      effectiveConfig: effective ? toEffectiveView(effective) : null,
      runtimeOverlay: this.prResolverOverlay,
      running: this.prResolver?.isRunning() ?? false,
      lastCycle: this.prResolver?.getLastCycleSummary() ?? null,
      state: stateSnap,
      prMachines: stateSnap?.pr_machines ?? {},
      recentEvents: getRecentPrResolverEvents(200),
      streamTails: this.snapshotStreamTails(),
    };
  }

  /**
   * Snapshot the per-PR rolling tails of high-frequency stream events. The
   * dashboard hook seeds `resolverStreams` / `grpcServerLogs` from this
   * payload on every hello/refresh — without it, those panels would always
   * show empty for any PR you didn't watch live.
   */
  private snapshotStreamTails(): Record<
    string,
    { resolverStream: string[]; grpcServerLog: string[] }
  > {
    const out: Record<
      string,
      { resolverStream: string[]; grpcServerLog: string[] }
    > = {};
    const prs = new Set<number>([
      ...this.prResolverStreamTails.keys(),
      ...this.prGrpcServerLogTails.keys(),
    ]);
    for (const pr of prs) {
      out[String(pr)] = {
        resolverStream: this.prResolverStreamTails.get(pr) ?? [],
        grpcServerLog: this.prGrpcServerLogTails.get(pr) ?? [],
      };
    }
    return out;
  }

  /**
   * Tee a single event into the appropriate per-PR rolling tail. Only fires
   * for the two high-volume event types — everything else is small enough to
   * live in the main replay buffer.
   */
  private appendPrStreamTail(event: {
    type: string;
    payload: Record<string, unknown>;
  }): void {
    const pr = typeof event.payload.pr === "number" ? event.payload.pr : null;
    if (pr === null) return;
    const line =
      typeof event.payload.line === "string" ? event.payload.line : null;
    if (!line) return;
    const target =
      event.type === "resolver_stream"
        ? this.prResolverStreamTails
        : event.type === "grpc_server_log"
          ? this.prGrpcServerLogTails
          : null;
    if (!target) return;
    const tail = target.get(pr) ?? [];
    tail.push(line);
    if (tail.length > PR_STREAM_TAIL_LIMIT) {
      tail.splice(0, tail.length - PR_STREAM_TAIL_LIMIT);
    }
    target.set(pr, tail);
  }

  /**
   * Wipe the appropriate per-PR rolling tail when a new "phase" starts so
   * the live-output panel doesn't keep showing lines from the prior cycle:
   *
   *   - `subtask_start`   → reset the resolver_stream tail (new Claude call)
   *   - `grpc_server_starting` → reset the grpc_server_log tail
   *
   * Without this, a rejected-then-requeued PR shows the previous Claude
   * session's final lines for the entire ~50s the new session is in flight,
   * which makes it look like nothing is happening.
   */
  private resetPrStreamTailIfBoundary(event: {
    type: string;
    payload: Record<string, unknown>;
  }): void {
    const pr = typeof event.payload.pr === "number" ? event.payload.pr : null;
    if (pr === null) return;
    if (event.type === "subtask_start") {
      this.prResolverStreamTails.delete(pr);
    } else if (event.type === "grpc_server_starting") {
      this.prGrpcServerLogTails.delete(pr);
    }
  }

  /** Broadcast a fresh pr-resolver:snapshot to every connected dashboard. */
  private broadcastPrResolverSnapshot(): void {
    this.broadcastControl(
      "pr-resolver:snapshot",
      this.buildPrResolverSnapshotPayload()
    );
  }

  /**
   * Debounced snapshot broadcast — fired from the event subscriber so a burst
   * of events (e.g. cycle_start + pr_start + subtask_start in the same tick)
   * coalesces into a single snapshot. 100ms is short enough that the UI feels
   * live, long enough to dedupe a typical event burst.
   */
  private prResolverSnapshotTimer: NodeJS.Timeout | null = null;
  private scheduleSnapshotBroadcast(): void {
    if (this.prResolverSnapshotTimer) return;
    this.prResolverSnapshotTimer = setTimeout(() => {
      this.prResolverSnapshotTimer = null;
      this.broadcastPrResolverSnapshot();
    }, 100);
    this.prResolverSnapshotTimer.unref();
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
    const cleared = this.state.recoverStaleSessions(BOOT_STALE_THRESHOLD_MS);
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
      } catch (err) {
        // eslint-disable-next-line no-console
        console.warn(
          `[supervisor] dropped malformed engine WS frame: ${err instanceof Error ? err.message : String(err)}`
        );
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

    // Push a PR Resolver snapshot too — recent events + persisted state +
    // current cycle status + the form fields the dashboard renders. Lets
    // the PrResolverPage render immediately without waiting for the next
    // emitted event.
    this.sendRaw(
      ws,
      JSON.stringify({
        type: "pr-resolver:snapshot",
        payload: this.buildPrResolverSnapshotPayload(),
      })
    );

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
      } catch (err) {
        // eslint-disable-next-line no-console
        console.warn(
          `[supervisor] dropped malformed dashboard WS frame: ${err instanceof Error ? err.message : String(err)}`
        );
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
    // runs:list is a read-only DB query the dashboard fires when the user
    // opens the "Past runs" dropdown. Historically only the engine answered
    // it, which meant the dropdown showed "No saved runs" whenever the
    // engine wasn't attached — exactly when the user most needs it (to pick
    // a run to resume). Serve it from the supervisor directly so it works
    // independent of engine state.
    if (msg.type === "runs:list") {
      try {
        const runs = await this.state.listRuns();
        const withHistory = await Promise.all(
          runs.map(async (r) => ({
            ...r,
            checkpoints: await this.state.getCheckpointHistory(r.runId),
          })),
        );
        try {
          dc.ws.send(
            JSON.stringify({
              type: "runs:list:response",
              payload: { runs: withHistory },
            }),
          );
        } catch {
          /* dashboard ws gone; nothing to do */
        }
      } catch (err) {
        try {
          dc.ws.send(
            JSON.stringify({
              type: "runs:list:response",
              payload: {
                runs: [],
                error: err instanceof Error ? err.message : String(err),
              },
            }),
          );
        } catch {
          /* ignore */
        }
      }
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
      // Engine is dead. Most messages legitimately can't proceed — drop
      // them. But two intents from the RunsPicker can *revive* the engine
      // instead of dying silently in the relay below:
      //   runs:resume { runId, startFrom? } — boot engine pointed at that run
      //   runs:new                          — boot engine for a fresh run
      // This unifies the dead-engine path with what a live engine already
      // does for these messages: emit __TENXGRACE_RESPAWN__ and exit so
      // the on-exit handler at line ~1227 calls startSession() with the
      // same StartIntent. Before this, clicking Resume on a crashed-out
      // session was a no-op (silent warn + drop), and the user had to
      // know to click "Start run" in the top bar first.
      if (msg.type === "runs:resume" || msg.type === "runs:new") {
        const sessionId = dc.sessionId;
        const payload = (msg.payload ?? {}) as {
          runId?: unknown;
          startFrom?: unknown;
        };
        const intent: StartIntent =
          msg.type === "runs:resume"
            ? {
                runId:
                  typeof payload.runId === "string" && payload.runId
                    ? payload.runId
                    : undefined,
                startFrom:
                  typeof payload.startFrom === "string"
                    ? payload.startFrom
                    : undefined,
              }
            : {};
        if (msg.type === "runs:resume" && !intent.runId) {
          // eslint-disable-next-line no-console
          console.warn(
            `[supervisor] runs:resume on dead engine for ${sessionId} missing runId — dropping`
          );
          return;
        }
        if (this.active.has(sessionId)) {
          // A spawn is already in flight (e.g. double-click on Resume);
          // let the first one land. startSession() is also idempotent for
          // a live pid, but short-circuiting here avoids broadcasting a
          // second "sessions:starting" and skips the getSession lookup.
          return;
        }
        this.broadcastControl("sessions:starting", {
          sessionId,
          via: msg.type,
        });
        try {
          await this.startSession(sessionId, intent);
        } catch (err) {
          this.broadcastControl("sessions:start:error", {
            sessionId,
            error: err instanceof Error ? err.message : String(err),
          });
        }
        return;
      }
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
      // ─── PR Resolver controls ────────────────────────────────────────
      case "pr-resolver:poll-now": {
        if (!this.prResolver) {
          this.send(ws, "pr-resolver:poll-now:error", {
            error: "PR Resolver is not enabled",
          });
          return;
        }
        if (this.prResolver.isRunning()) {
          this.send(ws, "pr-resolver:poll-now:error", {
            error: "Cycle already in progress",
          });
          return;
        }
        // Fire-and-forget so the WS reply is immediate; the dashboard sees
        // cycle_start / cycle_end events via the broadcast.
        void this.prResolver.runOnce().catch((err) => {
          // eslint-disable-next-line no-console
          console.error(
            `[supervisor] pr-resolver poll-now failed:`,
            err instanceof Error ? err.message : err
          );
        });
        this.send(ws, "pr-resolver:poll-now:ack", { ok: true });
        return;
      }
      case "pr-resolver:state:request": {
        if (!this.prResolver) {
          this.send(ws, "pr-resolver:state:response", {
            enabled: false,
          });
          return;
        }
        this.send(ws, "pr-resolver:state:response", {
          enabled: true,
          running: this.prResolver.isRunning(),
          lastCycle: this.prResolver.getLastCycleSummary(),
          state: this.prResolver.getStateSnapshot(),
        });
        return;
      }
      case "pr-resolver:configure": {
        const overlay = (payload.overlay ?? {}) as PrResolverRuntimeOverlay;
        const result = await this.reconfigurePrResolver(overlay);
        if (result.ok) {
          this.send(ws, "pr-resolver:configure:ack", { ok: true });
        } else {
          this.send(ws, "pr-resolver:configure:error", {
            errors: result.errors ?? ["unknown error"],
          });
        }
        return;
      }
      case "pr-resolver:toggle": {
        const enabled = !!payload.enabled;
        const overlay: PrResolverRuntimeOverlay = {
          ...this.prResolverOverlay,
          enabled,
        };
        const result = await this.reconfigurePrResolver(overlay);
        if (result.ok) {
          this.send(ws, "pr-resolver:configure:ack", { ok: true });
        } else {
          this.send(ws, "pr-resolver:configure:error", {
            errors: result.errors ?? ["unknown error"],
          });
        }
        return;
      }
      case "pr-resolver:approve": {
        if (!this.prResolver) {
          this.send(ws, "pr-resolver:approve:error", {
            error: "PR Resolver is not enabled",
          });
          return;
        }
        const prNumber = Number(payload.prNumber);
        if (!Number.isFinite(prNumber)) {
          this.send(ws, "pr-resolver:approve:error", {
            error: "Invalid prNumber",
          });
          return;
        }
        const note = typeof payload.note === "string" ? payload.note : undefined;
        const result = await this.prResolver.approvePr(prNumber, note);
        if (result.ok) {
          this.send(ws, "pr-resolver:approve:ack", { prNumber });
          this.broadcastPrResolverSnapshot();
        } else {
          this.send(ws, "pr-resolver:approve:error", {
            prNumber,
            error: result.error ?? "unknown",
          });
        }
        return;
      }
      case "pr-resolver:reject": {
        if (!this.prResolver) {
          this.send(ws, "pr-resolver:reject:error", {
            error: "PR Resolver is not enabled",
          });
          return;
        }
        const prNumber = Number(payload.prNumber);
        if (!Number.isFinite(prNumber)) {
          this.send(ws, "pr-resolver:reject:error", {
            error: "Invalid prNumber",
          });
          return;
        }
        const reason =
          typeof payload.reason === "string" ? payload.reason : undefined;
        const result = await this.prResolver.rejectPr(prNumber, reason);
        if (result.ok) {
          this.send(ws, "pr-resolver:reject:ack", { prNumber });
          this.broadcastPrResolverSnapshot();
        } else {
          this.send(ws, "pr-resolver:reject:error", {
            prNumber,
            error: result.error ?? "unknown",
          });
        }
        return;
      }
      case "pr-resolver:retry": {
        if (!this.prResolver) {
          this.send(ws, "pr-resolver:retry:error", {
            error: "PR Resolver is not enabled",
          });
          return;
        }
        const prNumber = Number(payload.prNumber);
        if (!Number.isFinite(prNumber)) {
          this.send(ws, "pr-resolver:retry:error", {
            error: "Invalid prNumber",
          });
          return;
        }
        const result = await this.prResolver.retryPr(prNumber);
        if (result.ok) {
          this.send(ws, "pr-resolver:retry:ack", { prNumber });
          this.broadcastPrResolverSnapshot();
        } else {
          this.send(ws, "pr-resolver:retry:error", {
            prNumber,
            error: result.error ?? "unknown",
          });
        }
        return;
      }
      case "pr-resolver:request_changes": {
        if (!this.prResolver) {
          this.send(ws, "pr-resolver:request_changes:error", {
            error: "PR Resolver is not enabled",
          });
          return;
        }
        const prNumber = Number(payload.prNumber);
        const feedback = typeof payload.feedback === "string" ? payload.feedback : "";
        if (!Number.isFinite(prNumber)) {
          this.send(ws, "pr-resolver:request_changes:error", {
            error: "Invalid prNumber",
          });
          return;
        }
        const result = await this.prResolver.requestChanges(prNumber, feedback);
        if (result.ok) {
          this.send(ws, "pr-resolver:request_changes:ack", { prNumber });
          this.broadcastPrResolverSnapshot();
        } else {
          this.send(ws, "pr-resolver:request_changes:error", {
            prNumber,
            error: result.error ?? "unknown",
          });
        }
        return;
      }
      case "pr-resolver:diff:request": {
        if (!this.prResolver) {
          this.send(ws, "pr-resolver:diff:response", {
            prNumber: Number(payload.prNumber),
            error: "PR Resolver is not enabled",
          });
          return;
        }
        const prNumber = Number(payload.prNumber);
        const machine = this.prResolver
          .getStateSnapshot()
          .pr_machines[String(prNumber)];
        this.send(ws, "pr-resolver:diff:response", {
          prNumber,
          diff: machine?.diffPreview ?? "",
          status: machine?.status ?? null,
        });
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
            // Spread first so wizard fields (authScheme, supportedFlows, etc.)
            // flow through. Specific keys below override / normalize.
            ...(initialTask as unknown as Record<string, unknown>),
            title: initialTask.title,
            description: initialTask.description,
            acceptanceCriteria: initialTask.acceptanceCriteria,
            projectRoot: session.projectRoot,
            sessionId,
            paymentMethod: initialTask.paymentMethod,
            targetConnectors: initialTask.targetConnectors,
            paymentMethodCategory:
              (initialTask as unknown as { category?: string }).category
              ?? initialTask.paymentMethodCategory,
            priority: initialTask.priority,
            runner: initialTask.runner,
            runnerModel: initialTask.runnerModel,
            connectorDocUrls: initialTask.connectorDocUrls ?? [],
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

    // Stop the PR Resolver before everything else — gives an in-flight cycle
    // a chance to finish gracefully (the service polls cancelled between
    // gates).
    if (this.prResolver) {
      this.prResolver.cancel();
      this.prResolverUnsub?.();
      try {
        await Promise.race([
          this.prResolverTask,
          sleep(TERM_GRACE_MS),
        ]);
      } catch {
        /* ignore — best-effort drain */
      }
    }

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

/**
 * Public projection of PrResolverConfig — only the fields the dashboard is
 * allowed to render in the settings form. Drops cargo commands and access
 * lists so they can't be changed from a browser tab.
 */
function toEffectiveView(cfg: PrResolverConfig) {
  return {
    enabled: cfg.enabled,
    autoApprove: cfg.autoApprove,
    githubRepo: cfg.githubRepo,
    trigger: cfg.trigger,
    pollInterval: cfg.pollInterval,
    maxConcurrent: cfg.maxConcurrent,
    maxBuildLoops: cfg.maxBuildLoops,
    maxCommentsPerCycle: cfg.maxCommentsPerCycle,
    grpcTestEnabled: cfg.grpcTestEnabled,
    grpcPort: cfg.grpcPort,
    grpcServerStartTimeoutMs: cfg.grpcServerStartTimeoutMs,
  };
}
