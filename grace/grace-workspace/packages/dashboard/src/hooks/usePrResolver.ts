import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Live state for the PR Resolver dashboard tab. Connects to the supervisor's
 * control WS, registers as a dashboard, and listens for `pr-resolver:*`
 * broadcasts. The hook owns the projected board model (Queued / In Progress /
 * Completed / Failed) so the page can render it directly.
 */

export type ProcessedThreadStatus = "fixed" | "failed" | "build_blocked";

export interface ProcessedThreadEntry {
  pr_number: number;
  processed_at: string;
  status: ProcessedThreadStatus;
  commit_sha?: string;
  path?: string;
  instruction_preview?: string;
  resolution_summary?: string;
  error?: string;
}

export interface BuildFailureEntry {
  branch: string;
  head_sha: string;
  failed_at: string;
  error: string;
}

export interface CycleSummary {
  cycle: number;
  total: number;
  fixed: number;
  failed: number;
  skipped: number;
  queued: number;
  startedAt: number;
  completedAt: number;
}

export interface BoardCard {
  /** Unique key for React (PR + connector or threadId). */
  key: string;
  pr: number;
  connector?: string;
  title: string;
  path?: string;
  detail?: string;
  steps: BoardStep[];
  status: "queued" | "in_progress" | "completed" | "failed";
  /** ISO timestamp of the last update. */
  updatedAt: number;
  commitSha?: string;
  error?: string;
}

export interface BoardStep {
  type: string;
  text: string;
  detail?: string;
  passed?: boolean;
  timestamp: number;
}

export interface PrResolverBoardState {
  queued: BoardCard[];
  inProgress: BoardCard[];
  completed: BoardCard[];
  failed: BoardCard[];
}

export interface EffectiveConfig {
  enabled: boolean;
  autoApprove: boolean;
  githubRepo: string;
  trigger: string;
  pollInterval: number;
  maxConcurrent: number;
  maxBuildLoops: number;
  maxCommentsPerCycle: number;
  grpcTestEnabled: boolean;
  grpcPort: number;
  grpcServerStartTimeoutMs: number;
}

export type PrMachineStatus =
  | "noticed"
  | "preparing"
  | "resolving"
  | "verifying"
  | "awaiting_approval"
  | "committing"
  | "pushed"
  | "rejected"
  | "failed";

export interface GrpcTestResultRecord {
  command: string;
  ok: boolean;
  exitCode: number | null;
  stdout: string;
  stderr: string;
  durationMs: number;
  timedOut: boolean;
}

export interface GrpcTestStepResult {
  name: string;
  method: string;
  command: string;
  ok: boolean;
  skipped: boolean;
  skipReason?: string;
  exitCode: number | null;
  stdout: string;
  stderr: string;
  durationMs: number;
  timedOut: boolean;
  captures: Record<string, unknown>;
  expectMisses: string[];
  renderedHeaders: Record<string, string>;
  renderedBody: unknown;
}

export interface GrpcTestPlan {
  tests: unknown[];
}

export interface PrMachine {
  prNumber: number;
  branch: string;
  status: PrMachineStatus;
  threadIds: string[];
  /** Parallel to threadIds; carries the trigger comment id we processed. */
  triggerCommentIds: string[];
  connectors: string[];
  remoteSha?: string;
  localSha?: string;
  diffPreview?: string;
  summary?: string;
  reason?: string;
  testPlan?: GrpcTestPlan;
  testStepResults?: GrpcTestStepResult[];
  testCommands?: string[];
  testResults?: GrpcTestResultRecord[];
  testCommandsSource?: "extracted" | "generated" | "none";
  testGenerationReply?: string;
  /** Reviewer-facing markdown summary generated async after cargo + grpc pass. */
  reviewSummary?: string;
  /** Lifecycle of the async review-summary call. */
  reviewSummaryStatus?: "generating" | "ready" | "failed";
  /** Error string when reviewSummaryStatus === "failed". */
  reviewSummaryError?: string;
  /** Worktree pool slot currently leased for this PR (when maxConcurrent > 1). */
  workerSlot?: number;
  startedAt: string;
  updatedAt: string;
}

export type RuntimeOverlay = Partial<EffectiveConfig>;

export interface UsePrResolverResult {
  /** Whether the supervisor reports the resolver as enabled in config. */
  enabled: boolean;
  /** Whether a cycle is currently running. */
  running: boolean;
  /** Connection status of the underlying WS. */
  controlStatus: "connecting" | "open" | "closed";
  /** Configured `owner/name`, surfaced for the header. */
  githubRepo: string;
  /** Configured trigger tag. */
  trigger: string;
  /** Last cycle summary (counters for the header chip). */
  lastCycle: CycleSummary | null;
  /** Live, projected board model. */
  board: PrResolverBoardState;
  /** Persisted state pulled from supervisor snapshot. */
  processedThreads: Record<string, ProcessedThreadEntry>;
  buildFailures: Record<string, BuildFailureEntry>;
  /** Currently effective config (config.yml defaults + runtime overlay). */
  effectiveConfig: EffectiveConfig | null;
  /** Only the fields explicitly overridden in `~/.byne/pr-resolver-config.json`. */
  runtimeOverlay: RuntimeOverlay;
  /** Per-PR state machines keyed by PR number. */
  prMachines: Record<string, PrMachine>;
  /** Whether commits are auto-pushed (no approval gate). Mirrors effectiveConfig.autoApprove. */
  autoApprove: boolean;
  /** Trigger a manual poll cycle. Resolves once the supervisor acks. */
  pollNow: () => void;
  /** Save a new runtime overlay (partial fields). Supervisor restarts the service. */
  updateConfig: (overlay: RuntimeOverlay) => void;
  /** Shortcut for the enable/disable toggle. */
  toggleEnabled: (enabled: boolean) => void;
  /** Flip auto-approve. Persists to runtime overlay. */
  setAutoApprove: (next: boolean) => void;
  /** Approve a pending PR. Server validates and pushes. */
  approvePr: (prNumber: number, note?: string) => void;
  /** Reject a pending PR. Server resets the worktree and replies on threads. */
  rejectPr: (prNumber: number, reason?: string) => void;
  /** Retry a failed/rejected PR — clears state so the next cycle picks it up. */
  retryPr: (prNumber: number) => void;
  /**
   * Send reviewer revision feedback for an awaiting-approval PR. The backend
   * resets the worktree, stashes the feedback on the machine, and re-queues
   * the threads so the next cycle re-runs the resolve loop with the feedback
   * rendered into the prompt.
   */
  requestChanges: (prNumber: number, feedback: string) => void;
  /** Pull the full stored diff for a PR (sent as `pr-resolver:diff:response`). */
  requestDiff: (prNumber: number) => void;
  /** Most recent diff:response payload (if any). */
  diffForPr: Record<string, { diff: string; status: PrMachineStatus | null }>;
  /** Rolling tail of `claude --verbose` stdout per PR (last ~300 lines). */
  resolverStreams: Record<string, string[]>;
  /** Rolling tail of `cargo run -p grpc-server` stdout/stderr per PR (cap 200). */
  grpcServerLogs: Record<string, string[]>;
  /** Last error from a server-rejected control message. */
  lastError: { kind: string; message: string } | null;
}

interface IncomingMessage {
  type: string;
  payload?: Record<string, unknown>;
}

interface SnapshotPayload {
  enabled?: boolean;
  autoApprove?: boolean;
  githubRepo?: string;
  trigger?: string;
  running?: boolean;
  lastCycle?: CycleSummary | null;
  effectiveConfig?: EffectiveConfig | null;
  runtimeOverlay?: RuntimeOverlay;
  state?: {
    processed_threads?: Record<string, ProcessedThreadEntry>;
    build_failures?: Record<string, BuildFailureEntry>;
    pr_machines?: Record<string, PrMachine>;
    last_poll?: string | null;
  };
  prMachines?: Record<string, PrMachine>;
  recentEvents?: Array<{
    type: string;
    timestamp: number;
    payload: Record<string, unknown>;
  }>;
  /**
   * Per-PR rolling tails of high-frequency stream events (resolver_stream,
   * grpc_server_log). These bypass `recentEvents` because they're too noisy
   * to share that 500-entry budget, so the supervisor maintains separate
   * per-PR buffers and ships them here. The hook seeds resolverStreams /
   * grpcServerLogs from this so the panels survive reconnects.
   */
  streamTails?: Record<
    string,
    { resolverStream?: string[]; grpcServerLog?: string[] }
  >;
}

function emptyBoard(): PrResolverBoardState {
  return { queued: [], inProgress: [], completed: [], failed: [] };
}

function machineStatusToColumn(
  status: PrMachineStatus
): "queued" | "in_progress" | "completed" | "failed" {
  switch (status) {
    case "noticed":
      return "queued";
    case "preparing":
    case "resolving":
    case "verifying":
    case "awaiting_approval":
    case "committing":
      return "in_progress";
    case "pushed":
      return "completed";
    case "rejected":
    case "failed":
      return "failed";
    default:
      return "queued";
  }
}

function machineDetail(
  machine: PrMachine,
  inflight: BoardCard | undefined
): string {
  if (machine.reason) return machine.reason;
  if (inflight?.detail) return inflight.detail;
  if (machine.status === "awaiting_approval")
    return `Awaiting your approval — ${machine.threadIds.length} thread(s)`;
  if (machine.status === "pushed" && machine.localSha)
    return `Pushed ${machine.localSha.slice(0, 8)}`;
  return machine.status;
}

export function usePrResolver(controlUrl: string): UsePrResolverResult {
  const [enabled, setEnabled] = useState(false);
  const [running, setRunning] = useState(false);
  const [controlStatus, setControlStatus] = useState<
    UsePrResolverResult["controlStatus"]
  >("connecting");
  const [githubRepo, setGithubRepo] = useState("");
  const [trigger, setTrigger] = useState("");
  const [lastCycle, setLastCycle] = useState<CycleSummary | null>(null);
  const [board, setBoard] = useState<PrResolverBoardState>(emptyBoard);
  const [processedThreads, setProcessedThreads] = useState<
    Record<string, ProcessedThreadEntry>
  >({});
  const [buildFailures, setBuildFailures] = useState<
    Record<string, BuildFailureEntry>
  >({});
  const [effectiveConfig, setEffectiveConfig] =
    useState<EffectiveConfig | null>(null);
  const [runtimeOverlay, setRuntimeOverlay] = useState<RuntimeOverlay>({});
  const [prMachines, setPrMachines] = useState<Record<string, PrMachine>>({});
  const [autoApprove, setAutoApproveState] = useState<boolean>(false);
  const [diffForPr, setDiffForPr] = useState<
    Record<string, { diff: string; status: PrMachineStatus | null }>
  >({});
  const [resolverStreams, setResolverStreams] = useState<
    Record<string, string[]>
  >({});
  const [grpcServerLogs, setGrpcServerLogs] = useState<
    Record<string, string[]>
  >({});
  const [lastError, setLastError] = useState<
    UsePrResolverResult["lastError"]
  >(null);

  const wsRef = useRef<WebSocket | null>(null);
  // `prCards` is the per-PR in-flight card model. Keyed by prNumber.
  const prCardsRef = useRef(new Map<number, BoardCard>());
  // Refs to the latest applyEvent / recomputeBoard callbacks. Without these,
  // the WS effect would have to include them in its deps; their identity
  // changes on every processedThreads update, which would close + reopen
  // the socket — visible as the "Connecting…" indicator flickering.
  const applyEventRef = useRef<(type: string, payload: Record<string, unknown> | undefined) => void>(
    () => {}
  );
  const recomputeBoardRef = useRef<() => void>(() => {});

  const recomputeBoard = useCallback(() => {
    const queued: BoardCard[] = [];
    const inProgress: BoardCard[] = [];
    const completed: BoardCard[] = [];
    const failed: BoardCard[] = [];

    // Source of truth = the persisted PrMachine map. Status comes from
    // machine.status (mapped to a column); steps + detail come from the
    // in-flight prCardsRef built off the live event stream. This avoids
    // the previous bug where a card stayed in "queued" because the
    // pr_done event had rotated out of the supervisor's replay buffer.
    const machinePrs = new Set<number>();
    for (const machine of Object.values(prMachines)) {
      machinePrs.add(machine.prNumber);
      const inflight = prCardsRef.current.get(machine.prNumber);
      const status = machineStatusToColumn(machine.status);
      const card: BoardCard = {
        key: `pr-${machine.prNumber}`,
        pr: machine.prNumber,
        connector:
          machine.connectors[0] ?? inflight?.connector ?? undefined,
        title: `PR #${machine.prNumber}`,
        path: undefined,
        detail: machineDetail(machine, inflight),
        steps: inflight?.steps ?? [],
        status,
        updatedAt: Date.parse(machine.updatedAt) || Date.now(),
        commitSha: machine.localSha ?? inflight?.commitSha,
        error: machine.reason ?? inflight?.error,
      };
      if (status === "queued") queued.push(card);
      else if (status === "in_progress") inProgress.push(card);
      else if (status === "completed") completed.push(card);
      else failed.push(card);
    }

    // In-flight cards that don't have a machine yet (rare — happens
    // between pr_start and the first machine upsert).
    for (const card of prCardsRef.current.values()) {
      if (machinePrs.has(card.pr)) continue;
      if (card.status === "queued") queued.push(card);
      else if (card.status === "in_progress") inProgress.push(card);
      else if (card.status === "completed") completed.push(card);
      else failed.push(card);
    }

    // Legacy entries: PRs that exist only in processed_threads (pre-machine
    // history). Render as terminal cards so the user still sees them.
    const machineOrInflightPrs = new Set<number>([
      ...machinePrs,
      ...Array.from(prCardsRef.current.keys()),
    ]);
    const completedByPr = new Map<number, BoardCard>();
    const failedByPr = new Map<number, BoardCard>();
    for (const [threadId, entry] of Object.entries(processedThreads)) {
      if (machineOrInflightPrs.has(entry.pr_number)) continue;
      const key = `pr-${entry.pr_number}-${threadId}`;
      const card: BoardCard = {
        key,
        pr: entry.pr_number,
        title: `PR #${entry.pr_number}`,
        path: entry.path,
        detail:
          entry.resolution_summary ||
          entry.instruction_preview ||
          entry.error ||
          "",
        steps: [],
        status:
          entry.status === "fixed"
            ? "completed"
            : entry.status === "build_blocked"
              ? "failed"
              : "failed",
        updatedAt: Date.parse(entry.processed_at) || 0,
        commitSha: entry.commit_sha,
        error: entry.error,
      };
      const target = card.status === "completed" ? completedByPr : failedByPr;
      const existing = target.get(entry.pr_number);
      if (!existing || card.updatedAt > existing.updatedAt) {
        target.set(entry.pr_number, card);
      }
    }
    completed.push(...completedByPr.values());
    failed.push(...failedByPr.values());

    // Sort each column most-recent-first
    const byRecent = (a: BoardCard, b: BoardCard) => b.updatedAt - a.updatedAt;
    queued.sort(byRecent);
    inProgress.sort(byRecent);
    completed.sort(byRecent);
    failed.sort(byRecent);

    setBoard({ queued, inProgress, completed, failed });
  }, [processedThreads, prMachines]);

  const upsertPrCard = useCallback(
    (pr: number, patch: Partial<BoardCard> & { stepToAdd?: BoardStep }) => {
      const existing =
        prCardsRef.current.get(pr) ??
        ({
          key: `pr-${pr}`,
          pr,
          title: `PR #${pr}`,
          steps: [],
          status: "queued" as const,
          updatedAt: Date.now(),
        } as BoardCard);
      const next: BoardCard = {
        ...existing,
        ...patch,
        steps: patch.stepToAdd
          ? [...existing.steps, patch.stepToAdd]
          : existing.steps,
        updatedAt: Date.now(),
      };
      prCardsRef.current.set(pr, next);
      recomputeBoard();
    },
    [recomputeBoard]
  );

  // Drive board recomputation when persisted state changes.
  useEffect(() => {
    recomputeBoard();
  }, [recomputeBoard]);

  const applyEvent = useCallback(
    (type: string, payload: Record<string, unknown> | undefined) => {
      const p = payload ?? {};
      const pr = typeof p.pr === "number" ? p.pr : undefined;
      const connector =
        typeof p.connector === "string" ? p.connector : undefined;
      // Snapshot replay passes the original event timestamp through `p.timestamp`.
      // Live broadcasts include it via supervisor's appendPrStreamTail teeing.
      // Fall back to Date.now() if neither is present (defensive).
      const ts =
        typeof p.timestamp === "number" ? (p.timestamp as number) : Date.now();

      switch (type) {
        case "pr-resolver:cycle_start":
          setRunning(true);
          break;
        case "pr-resolver:cycle_end":
          setRunning(false);
          if (p && typeof p.cycle === "number") {
            setLastCycle({
              cycle: Number(p.cycle),
              total: Number(p.total ?? 0),
              fixed: Number(p.fixed ?? 0),
              failed: Number(p.failed ?? 0),
              skipped: Number(p.skipped ?? 0),
              queued: Number(p.queued ?? 0),
              startedAt: Number(p.startedAt ?? 0),
              completedAt: Number(p.completedAt ?? Date.now()),
            });
          }
          break;
        case "pr-resolver:pr_queued":
          if (pr !== undefined) upsertPrCard(pr, { status: "queued" });
          break;
        case "pr-resolver:pr_start":
          if (pr !== undefined) {
            const count =
              typeof p.threadCount === "number" ? p.threadCount : 0;
            upsertPrCard(pr, {
              status: "in_progress",
              detail: `${count} comment(s) to fix`,
              stepToAdd: {
                type: "pr_start",
                text: "Started",
                detail: `${count} comment(s)`,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:pr_done":
          if (pr !== undefined) {
            const fixed = Number(p.fixed ?? 0);
            const failed = Number(p.failed ?? 0);
            upsertPrCard(pr, {
              status: failed > 0 ? "failed" : fixed > 0 ? "completed" : "failed",
              detail: `fixed=${fixed} failed=${failed} skipped=${p.skipped ?? 0}`,
            });
          }
          break;
        case "pr-resolver:pr_failed":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              status: "failed",
              error: String(p.error ?? "unknown"),
            });
          }
          break;
        case "pr-resolver:gate":
        case "pr-resolver:subtask_gate":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "gate",
                text: `${connector ? `[${connector}] ` : ""}${String(p.name ?? p.gate ?? "gate")}`,
                detail: String(p.detail ?? ""),
                passed: p.passed === true,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:subtask_start":
          if (pr !== undefined) {
            // Wipe the local Claude live-output buffer so a retry / requeue
            // doesn't keep streaming the prior cycle's final lines for the
            // ~50s the new session is in flight.
            setResolverStreams((prev) => {
              const key = String(pr);
              if (!prev[key]) return prev;
              const next = { ...prev };
              delete next[key];
              return next;
            });
            upsertPrCard(pr, {
              stepToAdd: {
                type: "subtask",
                text: `[${connector ?? "?"}] sub-task start`,
                detail: `${p.commentCount ?? "?"} comment(s)`,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:subtask_done":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "subtask",
                text: `[${connector ?? "?"}] committed`,
                detail: String(p.sha ?? "").slice(0, 8),
                passed: true,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:subtask_failed":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "subtask",
                text: `[${connector ?? "?"}] failed`,
                detail: String(p.error ?? ""),
                passed: false,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:build_fail":
        case "pr-resolver:clippy_fail":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "cargo",
                text: type === "pr-resolver:build_fail" ? "build fail" : "clippy fail",
                detail: String(p.outputTail ?? "").slice(-200),
                passed: false,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:build_pass":
        case "pr-resolver:clippy_pass":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "cargo",
                text: type === "pr-resolver:build_pass" ? "build pass" : "clippy pass",
                passed: true,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:grpc_test_extracted":
        case "pr-resolver:grpc_test_generated":
          if (pr !== undefined) {
            const verb = type === "pr-resolver:grpc_test_extracted" ? "extracted" : "generated";
            upsertPrCard(pr, {
              stepToAdd: {
                type: "grpc_test",
                text: `gRPC test commands ${verb}`,
                detail: `${p.count ?? 0} command(s)`,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:grpc_server_starting":
          if (pr !== undefined) {
            // Wipe the local grpc-server log so a re-run doesn't show the
            // prior cycle's server logs alongside the new ones.
            setGrpcServerLogs((prev) => {
              const key = String(pr);
              if (!prev[key]) return prev;
              const next = { ...prev };
              delete next[key];
              return next;
            });
            upsertPrCard(pr, {
              stepToAdd: {
                type: "grpc_test",
                text: `gRPC server starting (port ${p.port ?? "?"})`,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:grpc_server_ready":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "grpc_test",
                text: "gRPC server ready",
                passed: true,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:grpc_server_stopped":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "grpc_test",
                text: "gRPC server stopped",
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:grpc_test_command":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "grpc_test",
                text: "gRPC test command",
                detail: String(p.command ?? ""),
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:grpc_test_pass":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "grpc_test",
                text: `gRPC test pass (${p.count ?? 0} cmds)`,
                passed: true,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:grpc_test_fail":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "grpc_test",
                text: `gRPC test fail (${p.failed ?? "?"} / ${p.total ?? "?"})`,
                detail: String(p.reason ?? ""),
                passed: false,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:grpc_test_skipped":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "grpc_test",
                text: "gRPC test skipped",
                detail: String(p.reason ?? ""),
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:push_done":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              commitSha: String(p.sha ?? ""),
              stepToAdd: {
                type: "push",
                text: "pushed",
                detail: String(p.sha ?? "").slice(0, 8),
                passed: true,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:reply_posted":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "reply",
                text: "reply posted",
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:resolver_stream":
          if (pr !== undefined) {
            const line = String(p.line ?? "");
            if (line) {
              setResolverStreams((prev) => {
                const key = String(pr);
                const next = [...(prev[key] ?? []), line];
                if (next.length > 300) next.splice(0, next.length - 300);
                return { ...prev, [key]: next };
              });
            }
          }
          break;
        case "pr-resolver:grpc_server_log":
          if (pr !== undefined) {
            const line = String(p.line ?? "");
            if (line) {
              setGrpcServerLogs((prev) => {
                const key = String(pr);
                const next = [...(prev[key] ?? []), line];
                if (next.length > 200) next.splice(0, next.length - 200);
                return { ...prev, [key]: next };
              });
            }
          }
          break;
        case "pr-resolver:grpc_server_probe":
          if (pr !== undefined) {
            const attempt = Number(p.attempt ?? 0);
            const ok = p.ok === true;
            setGrpcServerLogs((prev) => {
              const key = String(pr);
              const note = ok
                ? `[probe #${attempt}] server is healthy ✓`
                : `[probe #${attempt}] not ready yet — retrying in 2s`;
              const next = [...(prev[key] ?? []), note];
              if (next.length > 200) next.splice(0, next.length - 200);
              return { ...prev, [key]: next };
            });
          }
          break;
        case "pr-resolver:grpc_test_plan_generated":
          if (pr !== undefined) {
            const steps = Number(p.steps ?? 0);
            upsertPrCard(pr, {
              stepToAdd: {
                type: "grpc_test",
                text: "test plan generated",
                detail: `${steps} step(s)`,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:grpc_test_step_start":
          if (pr !== undefined) {
            const name = String(p.name ?? "?");
            const dep = String(p.depends_on ?? "");
            upsertPrCard(pr, {
              stepToAdd: {
                type: "grpc_test",
                text: `▶ ${name}`,
                detail: dep ? `depends on: ${dep}` : undefined,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:grpc_test_step_pass":
          if (pr !== undefined) {
            const name = String(p.name ?? "?");
            const ms = Number(p.durationMs ?? 0);
            upsertPrCard(pr, {
              stepToAdd: {
                type: "grpc_test",
                text: `✓ ${name}`,
                detail: ms ? `${ms}ms` : undefined,
                passed: true,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:grpc_test_step_fail":
          if (pr !== undefined) {
            const name = String(p.name ?? "?");
            const exitCode = p.exitCode;
            const misses = Array.isArray(p.misses) ? p.misses : [];
            upsertPrCard(pr, {
              stepToAdd: {
                type: "grpc_test",
                text: `✗ ${name}`,
                detail:
                  (exitCode !== undefined ? `exit=${exitCode}` : "") +
                  (misses.length ? ` misses=${misses.join(",")}` : "") +
                  (typeof p.stderrTail === "string" && p.stderrTail
                    ? `\n${p.stderrTail}`
                    : ""),
                passed: false,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:grpc_test_step_skipped":
          if (pr !== undefined) {
            const name = String(p.name ?? "?");
            upsertPrCard(pr, {
              stepToAdd: {
                type: "grpc_test",
                text: `⊘ ${name}`,
                detail: String(p.reason ?? "(skipped)"),
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:review_summary_started":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "review_summary",
                text: "Generating reviewer summary…",
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:review_summary_generated":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "review_summary",
                text: "Reviewer summary ready",
                detail:
                  typeof p.summaryChars === "number"
                    ? `${p.summaryChars} chars`
                    : undefined,
                passed: true,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:review_summary_failed":
          if (pr !== undefined) {
            upsertPrCard(pr, {
              stepToAdd: {
                type: "review_summary",
                text: "Reviewer summary failed",
                detail: String(p.error ?? "unknown"),
                passed: false,
                timestamp: ts,
              },
            });
          }
          break;
        case "pr-resolver:state_changed":
          // Optional re-snapshot trigger — request state explicitly.
          wsRef.current?.send(
            JSON.stringify({ type: "pr-resolver:state:request", payload: {} })
          );
          break;
        case "pr-resolver:poll-now:error":
          setLastError({
            kind: "poll-now",
            message: String(p.error ?? "unknown"),
          });
          break;
      }
    },
    [upsertPrCard]
  );

  // Keep refs to the latest applyEvent / recomputeBoard. The WS effect
  // below uses these refs (not the callbacks directly) so its dependency
  // array can stay at `[controlUrl]` — otherwise every state update would
  // tear the socket down + reopen it (visible as "Connecting…" flicker).
  useEffect(() => {
    applyEventRef.current = applyEvent;
  }, [applyEvent]);
  useEffect(() => {
    recomputeBoardRef.current = recomputeBoard;
  }, [recomputeBoard]);

  useEffect(() => {
    let stopped = false;
    let retryTimer: number | null = null;

    const connect = () => {
      setControlStatus("connecting");
      const ws = new WebSocket(controlUrl);
      wsRef.current = ws;

      ws.onopen = () => {
        if (wsRef.current !== ws) return;
        setControlStatus("open");
        try {
          ws.send(
            JSON.stringify({
              type: "hello",
              payload: { role: "dashboard" },
            })
          );
        } catch {
          /* will retry */
        }
      };

      ws.onclose = () => {
        if (wsRef.current === ws) {
          setControlStatus("closed");
          wsRef.current = null;
          if (!stopped) {
            retryTimer = window.setTimeout(connect, 1500);
          }
        }
      };
      ws.onerror = () => {
        /* close handler reconnects */
      };

      ws.onmessage = (ev) => {
        let msg: IncomingMessage;
        try {
          msg = JSON.parse(ev.data);
        } catch {
          return;
        }
        if (msg.type === "pr-resolver:snapshot") {
          const p = (msg.payload ?? {}) as SnapshotPayload;
          setEnabled(p.enabled === true);
          setAutoApproveState(p.autoApprove === true);
          setGithubRepo(p.githubRepo ?? "");
          setTrigger(p.trigger ?? "");
          if (typeof p.running === "boolean") setRunning(p.running);
          setLastCycle(p.lastCycle ?? null);
          if (p.effectiveConfig) setEffectiveConfig(p.effectiveConfig);
          setRuntimeOverlay(p.runtimeOverlay ?? {});
          setPrMachines(p.prMachines ?? p.state?.pr_machines ?? {});
          if (p.state) {
            setProcessedThreads(p.state.processed_threads ?? {});
            setBuildFailures(p.state.build_failures ?? {});
          }
          // Replay recent events to rebuild in-flight cards. Use the refs
          // so we always invoke the latest closure even though this WS
          // effect was instantiated with an earlier `applyEvent`.
          if (p.recentEvents && Array.isArray(p.recentEvents)) {
            prCardsRef.current.clear();
            for (const e of p.recentEvents) {
              // Pass the event's original emission time through `timestamp`
              // on the payload, so the timeline shows when things actually
              // happened rather than collapsing every row to refresh-time.
              applyEventRef.current(`pr-resolver:${e.type}`, {
                ...e.payload,
                timestamp: e.timestamp,
              });
            }
            recomputeBoardRef.current();
          }
          // Seed the live-stream panels from the supervisor's per-PR rolling
          // tails. resolver_stream / grpc_server_log are NON_REPLAY_TYPES so
          // they aren't in `recentEvents` — without this seed, refreshing the
          // page mid-cycle wipes the panel.
          if (p.streamTails && typeof p.streamTails === "object") {
            const nextResolver: Record<string, string[]> = {};
            const nextGrpc: Record<string, string[]> = {};
            for (const [pr, tails] of Object.entries(p.streamTails)) {
              if (tails?.resolverStream?.length) {
                nextResolver[pr] = [...tails.resolverStream];
              }
              if (tails?.grpcServerLog?.length) {
                nextGrpc[pr] = [...tails.grpcServerLog];
              }
            }
            setResolverStreams(nextResolver);
            setGrpcServerLogs(nextGrpc);
          }
          return;
        }
        if (msg.type === "pr-resolver:diff:response") {
          const p = (msg.payload ?? {}) as {
            prNumber?: number;
            diff?: string;
            status?: PrMachineStatus | null;
            error?: string;
          };
          if (typeof p.prNumber === "number") {
            if (p.error) {
              setLastError({ kind: "diff", message: p.error });
            } else {
              setDiffForPr((prev) => ({
                ...prev,
                [String(p.prNumber)]: {
                  diff: p.diff ?? "",
                  status: p.status ?? null,
                },
              }));
            }
          }
          return;
        }
        if (
          msg.type === "pr-resolver:approve:ack" ||
          msg.type === "pr-resolver:reject:ack" ||
          msg.type === "pr-resolver:retry:ack" ||
          msg.type === "pr-resolver:request_changes:ack"
        ) {
          setLastError(null);
          return;
        }
        if (
          msg.type === "pr-resolver:approve:error" ||
          msg.type === "pr-resolver:reject:error" ||
          msg.type === "pr-resolver:retry:error" ||
          msg.type === "pr-resolver:request_changes:error"
        ) {
          const kind =
            msg.type === "pr-resolver:approve:error"
              ? "approve"
              : msg.type === "pr-resolver:reject:error"
                ? "reject"
                : msg.type === "pr-resolver:retry:error"
                  ? "retry"
                  : "request_changes";
          setLastError({
            kind,
            message: String(msg.payload?.error ?? "unknown"),
          });
          return;
        }
        if (msg.type === "pr-resolver:configure:ack") {
          setLastError(null);
          return;
        }
        if (msg.type === "pr-resolver:configure:error") {
          const errs = (msg.payload?.errors as string[] | undefined) ?? [
            "unknown",
          ];
          setLastError({ kind: "configure", message: errs.join("; ") });
          return;
        }
        if (msg.type === "pr-resolver:state:response") {
          const p = (msg.payload ?? {}) as SnapshotPayload;
          if (typeof p.running === "boolean") setRunning(p.running);
          if (p.lastCycle) setLastCycle(p.lastCycle);
          if (p.state) {
            setProcessedThreads(p.state.processed_threads ?? {});
            setBuildFailures(p.state.build_failures ?? {});
            // Without this, `state_changed` triggers a refetch but the PR
            // machine map (status / reviewSummary / testStepResults) never
            // actually updates — so approval gates and stage panels show
            // stale state until the next full snapshot.
            setPrMachines(p.state.pr_machines ?? {});
          }
          return;
        }
        if (msg.type.startsWith("pr-resolver:")) {
          applyEventRef.current(msg.type, msg.payload);
        }
      };
    };
    connect();
    return () => {
      stopped = true;
      if (retryTimer) window.clearTimeout(retryTimer);
      wsRef.current?.close();
      wsRef.current = null;
    };
    // Intentionally only depends on controlUrl — applyEvent / recomputeBoard
    // are accessed through refs to avoid tearing the socket down on every
    // state update.
  }, [controlUrl]);

  const pollNow = useCallback(() => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      setLastError({ kind: "poll-now", message: "WS not connected" });
      return;
    }
    ws.send(
      JSON.stringify({ type: "pr-resolver:poll-now", payload: {} })
    );
  }, []);

  const updateConfig = useCallback(
    (overlay: RuntimeOverlay) => {
      const ws = wsRef.current;
      if (!ws || ws.readyState !== WebSocket.OPEN) {
        setLastError({ kind: "configure", message: "WS not connected" });
        return;
      }
      // Send the full intended overlay so the supervisor doesn't have to
      // merge against a stale copy. Empty strings on the wire become
      // "clear this override" on the server side.
      ws.send(
        JSON.stringify({
          type: "pr-resolver:configure",
          payload: { overlay },
        })
      );
    },
    []
  );

  const toggleEnabled = useCallback(
    (next: boolean) => {
      const ws = wsRef.current;
      if (!ws || ws.readyState !== WebSocket.OPEN) {
        setLastError({ kind: "configure", message: "WS not connected" });
        return;
      }
      ws.send(
        JSON.stringify({
          type: "pr-resolver:toggle",
          payload: { enabled: next },
        })
      );
    },
    []
  );

  const setAutoApprove = useCallback(
    (next: boolean) => {
      const ws = wsRef.current;
      if (!ws || ws.readyState !== WebSocket.OPEN) {
        setLastError({ kind: "configure", message: "WS not connected" });
        return;
      }
      ws.send(
        JSON.stringify({
          type: "pr-resolver:configure",
          payload: { overlay: { autoApprove: next } },
        })
      );
    },
    []
  );

  const approvePr = useCallback((prNumber: number, note?: string) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      setLastError({ kind: "approve", message: "WS not connected" });
      return;
    }
    ws.send(
      JSON.stringify({
        type: "pr-resolver:approve",
        payload: { prNumber, note },
      })
    );
  }, []);

  const rejectPr = useCallback((prNumber: number, reason?: string) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      setLastError({ kind: "reject", message: "WS not connected" });
      return;
    }
    ws.send(
      JSON.stringify({
        type: "pr-resolver:reject",
        payload: { prNumber, reason },
      })
    );
  }, []);

  const requestDiff = useCallback((prNumber: number) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    ws.send(
      JSON.stringify({
        type: "pr-resolver:diff:request",
        payload: { prNumber },
      })
    );
  }, []);

  const retryPr = useCallback((prNumber: number) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      setLastError({ kind: "retry", message: "WS not connected" });
      return;
    }
    ws.send(
      JSON.stringify({
        type: "pr-resolver:retry",
        payload: { prNumber },
      })
    );
  }, []);

  const requestChanges = useCallback(
    (prNumber: number, feedback: string) => {
      const ws = wsRef.current;
      if (!ws || ws.readyState !== WebSocket.OPEN) {
        setLastError({ kind: "request_changes", message: "WS not connected" });
        return;
      }
      ws.send(
        JSON.stringify({
          type: "pr-resolver:request_changes",
          payload: { prNumber, feedback },
        })
      );
    },
    []
  );

  return {
    enabled,
    running,
    controlStatus,
    githubRepo,
    trigger,
    lastCycle,
    board,
    processedThreads,
    buildFailures,
    effectiveConfig,
    runtimeOverlay,
    prMachines,
    autoApprove,
    pollNow,
    updateConfig,
    toggleEnabled,
    setAutoApprove,
    approvePr,
    rejectPr,
    retryPr,
    requestChanges,
    requestDiff,
    diffForPr,
    resolverStreams,
    grpcServerLogs,
    lastError,
  };
}
