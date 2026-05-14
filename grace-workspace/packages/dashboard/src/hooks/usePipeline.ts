import { useEffect, useRef, useState } from "react";

export type CheckpointStatus =
  | "idle"
  | "running"
  | "passed"
  | "failed"
  | "skipped";

export interface CheckpointState {
  id: string;
  status: CheckpointStatus;
  retries: number;
  waiting?: { spec: unknown } | null;
  errors?: string[];
  lastEventTs?: string;
}

export interface LogLine {
  ts: string;
  checkpointId?: string;
  msg: string;
  level: string;
}

export interface RetryStep {
  checkpointId: string;
  rollbackTo: string;
  attempt: number;
  ts: string;
}

export type JourneyEventKind = "started" | "passed" | "failed" | "rollback";

export interface JourneyEvent {
  kind: JourneyEventKind;
  checkpointId: string;
  /** For rollback events: which checkpoint we rolled back to. */
  rollbackTo?: string;
  attempt: number;
  ts: string;
}

/**
 * Per-attempt record for retry history. Matches the core `AttemptRecord`
 * shape but kept duplicated here because the dashboard package doesn't
 * depend on @10xgrace/core.
 */
export interface AttemptRecord {
  artifacts: Record<string, unknown>;
  status: "passed" | "failed";
  errors?: string[];
  output?: string | null;
}

// Grace 2.3_codegen.md workflow: task → preflight → L2_planning → L3_analysis → implementation → compiler_check → grpc_test
// Removed: product_alignment, feature_research, design_gate, requirements, l4_gen, l4_review, design_match, cypress, playwright
export const PIPELINE: Array<{ id: string; name: string; type: "auto" | "human" }> = [
  { id: "task", name: "Task definition", type: "auto" },
  { id: "preflight", name: "Preflight setup", type: "auto" },
  { id: "l2_planning", name: "L2 Planning", type: "auto" },
  { id: "l2_review", name: "Human review: L2 plan", type: "human" },
  { id: "l3_analysis", name: "L3 Analysis", type: "auto" },
  { id: "l3_review", name: "Human review: L3 analysis", type: "human" },
  { id: "implementation", name: "Implementation", type: "auto" },
  { id: "compiler", name: "Compiler check", type: "auto" },
  { id: "compiler_check", name: "Compiler Check (Rust)", type: "auto" },
  { id: "grpc_test", name: "gRPC Test", type: "auto" },
  { id: "pr_review", name: "PR review", type: "human" },
  { id: "regression", name: "Regression testing", type: "auto" },
];

export type PipelineStatus = "idle" | "running" | "complete" | "aborted";

/**
 * usePipeline opens a WebSocket to the supervisor's control port (Phase 5
 * multiplex architecture) and registers as a dashboard client subscribed
 * to `sessionId`. The supervisor only forwards events tagged with that
 * sessionId, plus pipeline events from the matching engine. If `sessionId`
 * is omitted (legacy callers), the hook still opens the WS but won't
 * receive pipeline events — the supervisor only routes session-scoped
 * traffic.
 */
export function usePipeline(wsUrl: string, sessionId?: string) {
  const [runId, setRunId] = useState<string | undefined>();
  const [states, setStates] = useState<Record<string, CheckpointState>>(() => {
    const o: Record<string, CheckpointState> = {};
    for (const cp of PIPELINE) o[cp.id] = { id: cp.id, status: "idle", retries: 0 };
    return o;
  });
  const [logsByCp, setLogsByCp] = useState<Record<string, LogLine[]>>({});
  const [allLogs, setAllLogs] = useState<LogLine[]>([]);
  const [retries, setRetries] = useState<RetryStep[]>([]);
  const [journey, setJourney] = useState<JourneyEvent[]>([]);
  const [artifacts, setArtifacts] = useState<Record<string, unknown>>({});
  // Track per-attempt history per checkpoint for the RetryHistory selector.
  // Structure: { checkpointId: { retryAttempt: AttemptRecord } }
  // Populated from two sources:
  //   1. live `artifact:update` events as the pipeline runs
  //   2. `attempts:response` from the engine on connect/reload, replayed
  //      from the SQLite checkpoint_attempts table (survives WS replay
  //      buffer eviction and browser refresh)
  const [artifactHistory, setArtifactHistory] = useState<Record<string, Record<number, AttemptRecord>>>({});
  const [wsStatus, setWsStatus] = useState<"connecting" | "open" | "closed">(
    "connecting"
  );
  const [pipelineStatus, setPipelineStatus] = useState<PipelineStatus>("idle");
  const [abortReason, setAbortReason] = useState<string | null>(null);
  const [savedRuns, setSavedRuns] = useState<any[]>([]);
  const [lastRejection, setLastRejection] = useState<{
    checkpointId: string;
    reason: string;
    ts: string;
  } | null>(null);
  const [autoMode, setAutoModeState] = useState<{
    enabled: boolean;
    agentName?: string;
  }>({ enabled: false });
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    let cancelled = false;
    let retryTimer: number | undefined;

    const connect = () => {
      if (cancelled) return;
      setWsStatus("connecting");
      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;
      ws.onopen = () => {
        if (wsRef.current !== ws) return;
        setWsStatus("open");
        // Phase 5: tell the supervisor which session we're subscribing to
        // so it scopes pipeline events accordingly.
        try {
          ws.send(
            JSON.stringify({
              type: "hello",
              payload: { role: "dashboard", sessionId },
            })
          );
        } catch {
          /* ws will close + reconnect via the standard retry path */
        }
      };
      ws.onclose = () => {
        if (wsRef.current === ws) {
          setWsStatus("closed");
          retryTimer = window.setTimeout(connect, 1500);
        }
      };
      ws.onerror = () => {
        try {
          ws.close();
        } catch {
          /* ignore */
        }
      };
      ws.onmessage = (ev) => {
        try {
          const e = JSON.parse(ev.data);
          if (e.runId) {
            setRunId((prev) => {
              if (prev && prev !== e.runId) {
                // New run — reset all state so stale entries from the prior run disappear.
                const fresh: Record<string, CheckpointState> = {};
                for (const cp of PIPELINE)
                  fresh[cp.id] = { id: cp.id, status: "idle", retries: 0 };
                setStates(fresh);
                setLogsByCp({});
                setAllLogs([]);
                setRetries([]);
                setJourney([]);
                setArtifacts({});
                setPipelineStatus("idle");
                setAbortReason(null);
                setSavedRuns([]); // force the past-runs dropdown to re-fetch
              }
              return e.runId;
            });
          }
          const pushLog = (line: LogLine) => {
            setAllLogs((l) => [...l.slice(-1000), line]);
            if (line.checkpointId) {
              setLogsByCp((m) => ({
                ...m,
                [line.checkpointId!]: [
                  ...(m[line.checkpointId!] ?? []).slice(-500),
                  line,
                ],
              }));
            }
          };
          switch (e.type) {
            case "log":
              pushLog({
                ts: e.timestamp,
                checkpointId: e.checkpointId,
                msg: e.payload?.msg ?? "",
                level: e.payload?.level ?? "info",
              });
              break;
            case "checkpoint:status": {
              const newStatus = e.payload?.status;
              setStates((s) => {
                const prev = s[e.checkpointId] ?? { id: e.checkpointId, retries: 0, status: "idle" };
                if (newStatus === "running" || newStatus === "passed" || newStatus === "failed") {
                  setJourney((j) => [...j, {
                    kind: newStatus as JourneyEventKind,
                    checkpointId: e.checkpointId,
                    attempt: prev.retries,
                    ts: e.timestamp,
                  }]);
                }
                return {
                  ...s,
                  [e.checkpointId]: {
                    ...prev,
                    status: newStatus ?? "idle",
                    lastEventTs: e.timestamp,
                    waiting:
                      newStatus === "passed" || newStatus === "failed"
                        ? null
                        : (prev.waiting ?? null),
                  },
                };
              });
              if (newStatus === "running") {
                setPipelineStatus("running");
              }
              break;
            }
            case "checkpoint:retry":
              setRetries((r) => [
                ...r,
                {
                  checkpointId: e.checkpointId,
                  rollbackTo: e.payload?.rollbackTo,
                  attempt: e.payload?.attempt,
                  ts: e.timestamp,
                },
              ]);
              setJourney((j) => [...j, {
                kind: "rollback",
                checkpointId: e.checkpointId,
                rollbackTo: e.payload?.rollbackTo,
                attempt: e.payload?.attempt ?? 0,
                ts: e.timestamp,
              }]);
              setStates((s) => ({
                ...s,
                [e.checkpointId]: {
                  ...(s[e.checkpointId] ?? {
                    id: e.checkpointId,
                    retries: 0,
                    status: "failed",
                  }),
                  retries: e.payload?.attempt ?? 0,
                },
              }));
              break;
            case "human:waiting":
              setStates((s) => ({
                ...s,
                [e.checkpointId]: {
                  ...(s[e.checkpointId] ?? {
                    id: e.checkpointId,
                    retries: 0,
                    status: "running",
                  }),
                  status: "running",
                  waiting: { spec: e.payload?.spec },
                },
              }));
              break;
            case "artifact:update":
              if (e.payload?.artifacts && typeof e.payload.artifacts === "object") {
                // Merge into the global current-artifacts view so the
                // checkpoint-detail panel shows the latest result.
                setArtifacts((a) => ({ ...a, ...e.payload.artifacts }));
                // And store the full per-attempt record so RetryHistory can
                // navigate prior attempts. Engine always emits an envelope
                // with status/errors/output now, even for failure-with-no-
                // artifacts, so this branch fires for every completion.
                if (e.checkpointId && e.payload?.retryAttempt !== undefined) {
                  setArtifactHistory((h) => ({
                    ...h,
                    [e.checkpointId]: {
                      ...(h[e.checkpointId] ?? {}),
                      [e.payload.retryAttempt]: {
                        artifacts: e.payload.artifacts ?? {},
                        status: e.payload.status ?? "passed",
                        errors: e.payload.errors,
                        output: e.payload.output ?? null,
                      },
                    },
                  }));
                }
              }
              break;
            case "attempts:response": {
              const list = (e.payload?.attempts ?? []) as Array<{
                checkpointId: string;
                attemptIndex: number;
                status: "passed" | "failed";
                errors?: string[];
                output?: string | null;
                artifacts?: Record<string, unknown> | null;
              }>;
              if (list.length > 0) {
                setArtifactHistory((h) => {
                  const next: Record<string, Record<number, AttemptRecord>> = { ...h };
                  for (const a of list) {
                    next[a.checkpointId] = {
                      ...(next[a.checkpointId] ?? {}),
                      [a.attemptIndex]: {
                        artifacts: a.artifacts ?? {},
                        status: a.status,
                        errors: a.errors,
                        output: a.output ?? null,
                      },
                    };
                  }
                  return next;
                });
              }
              break;
            }
            case "runs:list:response":
              setSavedRuns(e.payload?.runs ?? []);
              break;
            case "auto-mode:state":
              setAutoModeState({
                enabled: !!e.payload?.enabled,
                agentName: e.payload?.agentName,
              });
              break;
            case "human:rejected":
              setLastRejection({
                checkpointId: e.checkpointId,
                reason: e.payload?.reason ?? "unknown",
                ts: e.timestamp,
              });
              pushLog({
                ts: e.timestamp,
                checkpointId: e.checkpointId,
                msg: `Submission rejected: ${e.payload?.reason ?? "unknown"}`,
                level: "error",
              });
              break;
            case "human:resolved":
            case "task:accepted":
              setLastRejection(null);
              setStates((s) => ({
                ...s,
                [e.checkpointId]: {
                  ...(s[e.checkpointId] ?? {
                    id: e.checkpointId,
                    retries: 0,
                    status: "running",
                  }),
                  waiting: null,
                },
              }));
              if (e.type === "task:accepted" && e.payload?.task) {
                setArtifacts((a) => ({ ...a, task: e.payload.task }));
              }
              break;
            case "task:rejected":
              pushLog({
                ts: e.timestamp,
                checkpointId: "task",
                msg: `Task rejected: ${e.payload?.reason ?? ""}`,
                level: "error",
              });
              break;
            case "pipeline:complete":
              setPipelineStatus("complete");
              pushLog({
                ts: e.timestamp,
                msg: "Pipeline complete ✓",
                level: "success",
              });
              break;
            case "pipeline:abort":
              setPipelineStatus("aborted");
              setAbortReason(e.payload?.error ?? "unknown");
              pushLog({
                ts: e.timestamp,
                msg: `Pipeline aborted: ${e.payload?.error ?? ""}`,
                level: "error",
              });
              break;
          }
        } catch {
          /* ignore */
        }
      };
    };
    connect();
    return () => {
      cancelled = true;
      if (retryTimer) window.clearTimeout(retryTimer);
      wsRef.current?.close();
    };
  }, [wsUrl, sessionId]);

  // Whenever the WS reaches "open" with a known runId (initial connect, or
  // reconnect after a drop), ask the engine to replay attempt history from
  // SQLite. This is the path that survives browser reloads — the live
  // artifact:update events sit in a 500-event replay buffer that fills up
  // with log spam, so we can't rely on replay alone.
  useEffect(() => {
    if (wsStatus !== "open" || !runId) return;
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    try {
      ws.send(JSON.stringify({ type: "attempts:request", payload: { runId } }));
    } catch {
      /* ignore — engine will keep streaming live events anyway */
    }
  }, [wsStatus, runId]);

  const send = (type: string, payload?: unknown) => {
    const ws = wsRef.current;
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type, payload }));
      return true;
    }
    return false;
  };

  return {
    runId,
    states,
    logsByCp,
    allLogs,
    retries,
    journey,
    artifacts,
    artifactHistory,
    wsStatus,
    pipelineStatus,
    abortReason,
    savedRuns,
    lastRejection,
    autoMode,
    send,
  };
}
