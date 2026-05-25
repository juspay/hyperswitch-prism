import { WebSocketServer, type WebSocket as NodeWebSocket } from "ws";

/**
 * Minimal stand-in for the real SessionSupervisor — speaks just enough
 * of the dashboard's WS protocol to drive Playwright tests without
 * cargo, GitHub, or Claude in the loop.
 *
 * Protocol surface used by the dashboard (see
 * `packages/dashboard/src/hooks/usePrResolver.ts`):
 *   ← `{ type: "hello", payload: { role: "dashboard" } }`
 *   → `{ type: "pr-resolver:snapshot", payload: SnapshotPayload }`
 *   → `{ type: "pr-resolver:<event>", payload: ... }`         (broadcast)
 *   ← `{ type: "pr-resolver:approve", payload: { prNumber } }`
 *   → `{ type: "pr-resolver:approve:ack", payload: { prNumber } }`
 *
 * Each test instance configures `snapshot` (the payload sent on hello)
 * and uses `sendEvent` to inject events as the test progresses. Inbound
 * messages from the dashboard surface as `inbound` for assertion.
 */

export interface FakeSnapshotPayload {
  enabled?: boolean;
  autoApprove?: boolean;
  githubRepo?: string;
  trigger?: string;
  running?: boolean;
  lastCycle?: unknown;
  effectiveConfig?: Record<string, unknown> | null;
  runtimeOverlay?: Record<string, unknown>;
  prMachines?: Record<string, unknown>;
  state?: {
    processed_threads?: Record<string, unknown>;
    build_failures?: Record<string, unknown>;
    pr_machines?: Record<string, unknown>;
    last_poll?: string | null;
  };
  recentEvents?: Array<{
    type: string;
    timestamp: number;
    payload: Record<string, unknown>;
  }>;
  streamTails?: Record<
    string,
    { resolverStream?: string[]; grpcServerLog?: string[] }
  >;
}

export interface InboundRecord {
  type: string;
  payload?: Record<string, unknown>;
  receivedAt: number;
}

export class FakeSupervisor {
  private wss: WebSocketServer | null = null;
  private clients = new Set<NodeWebSocket>();
  readonly inbound: InboundRecord[] = [];
  snapshot: FakeSnapshotPayload = defaultSnapshot();

  constructor(private readonly port: number) {}

  async start(): Promise<void> {
    return new Promise((resolve, reject) => {
      const wss = new WebSocketServer({ host: "127.0.0.1", port: this.port });
      wss.once("listening", () => {
        this.wss = wss;
        resolve();
      });
      wss.once("error", reject);
      wss.on("connection", (ws) => this.attach(ws));
    });
  }

  async stop(): Promise<void> {
    for (const ws of this.clients) {
      try {
        ws.close();
      } catch {
        /* ignore */
      }
    }
    this.clients.clear();
    if (!this.wss) return;
    await new Promise<void>((resolve) => {
      this.wss?.close(() => resolve());
    });
    this.wss = null;
  }

  /** Push an event to every connected dashboard. */
  sendEvent(type: string, payload: Record<string, unknown>): void {
    const frame = JSON.stringify({
      type: `pr-resolver:${type}`,
      payload: { ...payload, timestamp: Date.now() },
    });
    for (const ws of this.clients) {
      if (ws.readyState === ws.OPEN) ws.send(frame);
    }
  }

  /** Push a refreshed snapshot to every connected dashboard. */
  rebroadcastSnapshot(): void {
    const frame = JSON.stringify({
      type: "pr-resolver:snapshot",
      payload: this.snapshot,
    });
    for (const ws of this.clients) {
      if (ws.readyState === ws.OPEN) ws.send(frame);
    }
  }

  /** Convenience: replay an event-or-snapshot script with small delays. */
  async play(
    script: Array<{
      delayMs?: number;
      type: "snapshot" | string;
      patch?: (snap: FakeSnapshotPayload) => void;
      payload?: Record<string, unknown>;
    }>
  ): Promise<void> {
    for (const step of script) {
      if (step.delayMs) await sleep(step.delayMs);
      if (step.type === "snapshot") {
        step.patch?.(this.snapshot);
        this.rebroadcastSnapshot();
      } else {
        this.sendEvent(step.type, step.payload ?? {});
      }
    }
  }

  /** Wait until at least one dashboard has connected & said hello. */
  async waitForDashboard(timeoutMs = 5_000): Promise<void> {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      if (this.clients.size > 0) return;
      await sleep(50);
    }
    throw new Error("No dashboard connected within timeout");
  }

  private attach(ws: NodeWebSocket): void {
    this.clients.add(ws);
    ws.on("message", (raw) => {
      let parsed: { type?: string; payload?: Record<string, unknown> };
      try {
        parsed = JSON.parse(raw.toString());
      } catch {
        return;
      }
      const type = String(parsed.type ?? "");
      this.inbound.push({
        type,
        payload: parsed.payload,
        receivedAt: Date.now(),
      });

      // Mimic supervisor: dashboard's hello triggers the snapshot push.
      if (type === "hello") {
        ws.send(
          JSON.stringify({
            type: "pr-resolver:snapshot",
            payload: this.snapshot,
          })
        );
        return;
      }
      // Cheap acks so the UI doesn't show errors.
      if (type === "pr-resolver:approve") {
        ws.send(
          JSON.stringify({
            type: "pr-resolver:approve:ack",
            payload: parsed.payload ?? {},
          })
        );
        return;
      }
      if (type === "pr-resolver:reject") {
        ws.send(
          JSON.stringify({
            type: "pr-resolver:reject:ack",
            payload: parsed.payload ?? {},
          })
        );
        return;
      }
      if (type === "pr-resolver:retry") {
        ws.send(
          JSON.stringify({
            type: "pr-resolver:retry:ack",
            payload: parsed.payload ?? {},
          })
        );
        return;
      }
      if (type === "pr-resolver:configure") {
        ws.send(
          JSON.stringify({
            type: "pr-resolver:configure:ack",
            payload: {},
          })
        );
        return;
      }
      if (type === "pr-resolver:diff:request") {
        const prNumber = Number(parsed.payload?.prNumber ?? 0);
        ws.send(
          JSON.stringify({
            type: "pr-resolver:diff:response",
            payload: {
              prNumber,
              diff: "diff --git a/foo.rs b/foo.rs\n+pretend diff",
              status: this.snapshotMachineStatus(prNumber),
            },
          })
        );
        return;
      }
    });
    ws.on("close", () => {
      this.clients.delete(ws);
    });
  }

  private snapshotMachineStatus(prNumber: number): string | null {
    const machine = (this.snapshot.prMachines ?? {})[String(prNumber)] as
      | { status?: string }
      | undefined;
    return machine?.status ?? null;
  }
}

export function defaultSnapshot(): FakeSnapshotPayload {
  return {
    enabled: true,
    autoApprove: false,
    githubRepo: "juspay/hyperswitch-prism",
    trigger: "@grace fix",
    running: false,
    lastCycle: null,
    effectiveConfig: {
      enabled: true,
      autoApprove: false,
      githubRepo: "juspay/hyperswitch-prism",
      trigger: "@grace fix",
      pollInterval: 60,
      maxConcurrent: 1,
      maxBuildLoops: 3,
      maxCommentsPerCycle: 5,
      grpcTestEnabled: true,
      grpcPort: 8000,
      grpcServerStartTimeoutMs: 600_000,
    },
    runtimeOverlay: {},
    prMachines: {},
    state: {
      processed_threads: {},
      build_failures: {},
      pr_machines: {},
      last_poll: null,
    },
    recentEvents: [],
  };
}

export function makeFakeMachine(input: {
  prNumber: number;
  status:
    | "noticed"
    | "preparing"
    | "resolving"
    | "verifying"
    | "awaiting_approval"
    | "committing"
    | "pushed"
    | "rejected"
    | "failed";
  branch?: string;
  threadIds?: string[];
  connectors?: string[];
  summary?: string;
  reason?: string;
  diffPreview?: string;
  localSha?: string;
}): Record<string, unknown> {
  const now = new Date().toISOString();
  return {
    prNumber: input.prNumber,
    branch: input.branch ?? `feat/pr-${input.prNumber}`,
    status: input.status,
    threadIds: input.threadIds ?? [`thread-${input.prNumber}-1`],
    triggerCommentIds: [`tc-${input.prNumber}-1`],
    connectors: input.connectors ?? ["adyen"],
    summary: input.summary,
    reason: input.reason,
    diffPreview: input.diffPreview,
    localSha: input.localSha,
    startedAt: now,
    updatedAt: now,
  };
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}
