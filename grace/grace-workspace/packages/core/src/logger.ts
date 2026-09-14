import { WebSocket } from "ws";
import { nowIso } from "./utils.js";
import type { CheckpointId, CheckpointStatus } from "./types.js";

type Level = "info" | "warn" | "error" | "success" | "debug";

const COLORS: Record<Level, string> = {
  info: "\x1b[36m",
  warn: "\x1b[33m",
  error: "\x1b[31m",
  success: "\x1b[32m",
  debug: "\x1b[90m",
};
const RESET = "\x1b[0m";

export interface EventEnvelope {
  runId: string;
  /** Tagged on every outbound event so the supervisor can fan it out only to
   * dashboards subscribed to this session. Filled in by the bus, callers
   * never need to set it. */
  sessionId?: string;
  checkpointId?: CheckpointId;
  timestamp: string;
  type: string;
  payload?: unknown;
}

type InboundHandler = (msg: { type: string; payload?: unknown }) => void;

/**
 * Bidirectional event bus that connects an engine child process to the
 * supervisor's control WebSocket.
 *
 * Phase 5 architecture: every engine is a *client* that connects outbound
 * to `controlWsUrl` (the supervisor's port, e.g. ws://localhost:3334) and
 * registers itself with a `hello` frame. From there, the engine simply
 * emits envelopes; the supervisor multicasts them to dashboard clients
 * subscribed to this session and routes inbound messages from those
 * dashboards back to us.
 *
 * Public API matches the previous PipelineEventBus exactly so engine code
 * (PipelineEngine, run.ts) doesn't need to change. The `controlWsUrl` arg
 * replaces the previous `wsPort` arg — when undefined, the bus runs in
 * "no-op" mode (events are still buffered locally so log() prints) for the
 * dashboard-disabled CLI path.
 */
export class PipelineEventBus {
  private ws?: WebSocket;
  private runId: string;
  private sessionId?: string;
  private inboundHandlers = new Set<InboundHandler>();
  private outboundQueue: string[] = [];
  /** Local snapshot of recent events. Useful for in-engine debugging; the
   *  authoritative replay buffer for late-connecting dashboards now lives
   *  on the supervisor. */
  private lastSnapshot: Array<EventEnvelope> = [];
  private closed = false;

  constructor(runId: string, controlWsUrl?: string, sessionId?: string) {
    this.runId = runId;
    this.sessionId = sessionId;
    if (controlWsUrl) {
      this.connect(controlWsUrl);
    }
  }

  private connect(url: string): void {
    let ws: WebSocket;
    try {
      ws = new WebSocket(url);
    } catch (err) {
      // eslint-disable-next-line no-console
      console.error(`[events] failed to construct WS to ${url}:`, err);
      return;
    }
    this.ws = ws;

    ws.on("open", () => {
      // Identify ourselves so the supervisor can route by sessionId.
      // Same envelope shape as the dashboard hello: `{type, payload}`.
      const hello = JSON.stringify({
        type: "hello",
        payload: {
          role: "engine",
          sessionId: this.sessionId,
          runId: this.runId,
        },
      });
      try {
        ws.send(hello);
      } catch {
        /* will be retried on reconnect */
      }
      // Drain anything that emit() queued before the socket opened.
      while (this.outboundQueue.length > 0) {
        const msg = this.outboundQueue.shift()!;
        try {
          ws.send(msg);
        } catch {
          /* re-queue and stop draining */
          this.outboundQueue.unshift(msg);
          break;
        }
      }
    });

    ws.on("message", (raw) => {
      let parsed: { type: string; payload?: unknown };
      try {
        parsed = JSON.parse(raw.toString()) as {
          type: string;
          payload?: unknown;
        };
      } catch {
        return;
      }
      // The supervisor injects { sessionId } onto frames it relays to us.
      // Strip the envelope down to the shape the existing handlers expect.
      for (const h of this.inboundHandlers) h(parsed);
    });

    ws.on("close", () => {
      if (this.closed) return;
      // The supervisor going down or the control link dropping isn't a
      // fatal engine event — we just stop forwarding. The supervisor's
      // child-exit / heartbeat reaper will tear us down properly.
      // eslint-disable-next-line no-console
      console.log(`[events] control WS closed`);
    });

    ws.on("error", (err) => {
      // eslint-disable-next-line no-console
      console.error(`[events] control WS error:`, err);
    });
  }

  onInbound(handler: InboundHandler): () => void {
    this.inboundHandlers.add(handler);
    return () => this.inboundHandlers.delete(handler);
  }

  waitFor<T = unknown>(type: string, timeoutMs?: number): Promise<T> {
    return new Promise((resolve, reject) => {
      const off = this.onInbound((msg) => {
        if (msg.type === type) {
          off();
          if (timer) clearTimeout(timer);
          resolve(msg.payload as T);
        }
      });
      let timer: NodeJS.Timeout | undefined;
      if (timeoutMs && timeoutMs > 0) {
        timer = setTimeout(() => {
          off();
          reject(new Error(`Timed out waiting for inbound ${type}`));
        }, timeoutMs);
      }
    });
  }

  emit(type: string, checkpointId?: CheckpointId, payload?: unknown) {
    const envelope: EventEnvelope = {
      runId: this.runId,
      sessionId: this.sessionId,
      checkpointId,
      timestamp: nowIso(),
      type,
      payload,
    };
    this.lastSnapshot.push(envelope);
    if (this.lastSnapshot.length > 500) this.lastSnapshot.shift();
    const serialized = JSON.stringify(envelope);
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      try {
        this.ws.send(serialized);
      } catch {
        this.outboundQueue.push(serialized);
      }
    } else if (this.ws) {
      // Connecting or closed — buffer. We replay on open. Cap to keep
      // memory bounded; oldest events drop first.
      this.outboundQueue.push(serialized);
      if (this.outboundQueue.length > 500) this.outboundQueue.shift();
    }
    // No `this.ws` at all = dashboard-disabled mode; we still print logs.
  }

  log(
    checkpointId: CheckpointId | "pipeline",
    msg: string,
    level: Level = "info"
  ) {
    const ts = nowIso();
    const line = `${COLORS[level]}${ts} [${checkpointId}] ${level.toUpperCase()}${RESET} ${msg}`;
    // eslint-disable-next-line no-console
    console.log(line);
    this.emit("log", checkpointId === "pipeline" ? undefined : checkpointId, {
      msg,
      level,
    });
  }

  emitCheckpoint(
    type: "checkpoint:start" | "checkpoint:pass" | "checkpoint:fail" | "checkpoint:retry",
    checkpointId: CheckpointId,
    extra?: Record<string, unknown>
  ) {
    this.emit(type, checkpointId, extra);
  }

  emitStatus(checkpointId: CheckpointId, status: CheckpointStatus) {
    this.emit("checkpoint:status", checkpointId, { status });
  }

  emitHumanWaiting(checkpointId: CheckpointId, spec: unknown) {
    this.emit("human:waiting", checkpointId, { spec });
  }

  emitHumanResolved(checkpointId: CheckpointId, decision: string) {
    this.emit("human:resolved", checkpointId, { decision });
  }

  close() {
    this.closed = true;
    try {
      this.ws?.close();
    } catch {
      /* ignore */
    }
  }
}
