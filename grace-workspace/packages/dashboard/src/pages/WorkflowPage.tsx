import { useEffect, useMemo, useRef, useState } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";
import { usePipeline, PIPELINE } from "../hooks/usePipeline";
import type { JourneyEvent } from "../hooks/usePipeline";
import { useSessions, type SessionRecord } from "../hooks/useSessions";
import { Sidebar } from "../components/Sidebar";
import { CheckpointDetail } from "../components/CheckpointDetail";
import { LogPanel } from "../components/LogPanel";
import { RunsPicker } from "../components/RunsPicker";
import type { SubmittedTask } from "../components/TaskForm";
import { T } from "../theme";

const CONTROL_WS_PORT =
  (import.meta.env.VITE_WS_PORT as string | undefined) ?? "3142";
const CONTROL_WS_URL = `ws://${location.hostname}:${CONTROL_WS_PORT}`;

/**
 * Workflow view for a single session. The supervisor's control WS gives us
 * the session record (including `wsPort` once a child engine is alive); we
 * forward that port down to `usePipeline` so this component speaks directly
 * to the per-session engine. When the session is idle (no engine), we ask
 * the supervisor to start one and wait for the response before mounting
 * the pipeline UI.
 */
export function WorkflowPage() {
  const { sessionId = "default" } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  const location = useLocation();
  // Phase 8: distinguish "just-created session — auto-spawn an engine"
  // from "navigated in to inspect prior state — leave the page alone".
  // location.state is wiped by a hard refresh, which is exactly what we
  // want: refreshes should not re-trigger an autostart and destroy the
  // session's post-run replay buffer view.
  const autostartRequested =
    (location.state as { autostart?: boolean } | null)?.autostart === true;
  const {
    sessions,
    controlStatus,
    startSession,
    stopSession,
    archiveSession,
    deleteSession,
  } = useSessions(CONTROL_WS_URL);

  const session = useMemo(
    () => sessions.find((s) => s.sessionId === sessionId),
    [sessions, sessionId]
  );

  // Auto-start a child engine when this page mounts on an idle session.
  // Phase 5: liveness is signalled by `session.pid` (no per-session ws_port
  // anymore — everything multiplexes through the supervisor's control WS).
  // Phase 8: only auto-spawn when the user *intends* a fresh run — signaled
  // by `location.state.autostart` set by Homepage's create-flow navigate.
  // Click-throughs and refreshes don't pass autostart, so prior-run state
  // survives. The user can still kick off a new run via the explicit
  // "Start run" / "Start new run" button in TopBar.
  const startedRef = useRef(false);
  useEffect(() => {
    if (!autostartRequested) return;
    if (controlStatus !== "open") return;
    if (!session) return;
    if (session.status === "running" && session.pid != null) return;
    if (session.status === "archived") return;
    if (startedRef.current) return;
    startedRef.current = true;
    void startSession(sessionId);
  }, [autostartRequested, controlStatus, session, sessionId, startSession]);

  // The pipeline-event WS is the supervisor's control port, same one
  // useSessions uses. usePipeline sends a `hello {role:'dashboard',
  // sessionId}` so the supervisor scopes events to this session.
  const pipelineWsUrl = CONTROL_WS_URL;

  // Phase 7: only show WaitingForEngine for genuine error states. Once the
  // session exists and isn't archived, render WorkflowPageInner immediately
  // so post-run state (Pipeline aborted / Pipeline complete + the failed
  // checkpoint highlight, logs, and journey) survives the engine exiting.
  if (controlStatus !== "open" || !session || session.status === "archived") {
    return (
      <WaitingForEngine
        sessionId={sessionId}
        session={session ?? null}
        controlStatus={controlStatus}
        onBackToHome={() => navigate("/")}
      />
    );
  }

  const engineAlive = session.pid != null;

  return (
    <WorkflowPageInner
      // Don't include currentRunId in the key — that flips to null on engine
      // exit and would force a remount that wipes pipelineStatus / logs /
      // journey. usePipeline already resets per-run state internally when
      // it sees a fresh runId arrive, so a stable key is correct.
      key={sessionId}
      wsUrl={pipelineWsUrl}
      sessionId={sessionId}
      session={session}
      engineAlive={engineAlive}
      onStartNewRun={() => {
        // Re-arm the auto-start guard so the existing useEffect doesn't
        // race with us, then ask the supervisor for a fresh child.
        startedRef.current = true;
        void startSession(sessionId);
      }}
      onBackToHome={() => navigate("/")}
      onStopEngine={() => stopSession(sessionId)}
      onArchive={() => {
        if (!confirm(`Archive session "${session.name}"?`)) return;
        archiveSession(sessionId);
        navigate("/");
      }}
      onDelete={() => {
        if (
          !confirm(
            `Delete session "${session.name}"? This removes its worktree on disk.`
          )
        )
          return;
        deleteSession(sessionId);
        navigate("/");
      }}
    />
  );
}

function WaitingForEngine({
  sessionId,
  session,
  controlStatus,
  onBackToHome,
}: {
  sessionId: string;
  session: SessionRecord | null;
  controlStatus: "connecting" | "open" | "closed";
  onBackToHome: () => void;
}) {
  const message =
    controlStatus !== "open"
      ? `Connecting to supervisor (${controlStatus})…`
      : !session
        ? `Session "${sessionId}" not found.`
        : session.status === "archived"
          ? `Session "${session.name}" is archived.`
          : `Spawning engine for "${session.name}"…`;
  return (
    <div
      style={{
        height: "100vh",
        background: T.bg,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 14,
        color: T.text,
      }}
    >
      <div style={{ fontSize: 14, color: T.textMuted }}>{message}</div>
      <button
        onClick={onBackToHome}
        style={{
          padding: "7px 14px",
          borderRadius: 6,
          border: `1px solid ${T.border}`,
          background: T.bgElev,
          color: T.text,
          fontSize: 12,
          cursor: "pointer",
        }}
      >
        ← Back to sessions
      </button>
    </div>
  );
}

interface WorkflowInnerProps {
  wsUrl: string;
  sessionId: string;
  session: SessionRecord;
  /** Phase 7: drives the Cancel-vs-Start-new-run toggle in TopBar and
   *  gates the TaskForm submit button. True when an engine child is
   *  attached for this session; false post-exit or pre-spawn. */
  engineAlive: boolean;
  onStartNewRun: () => void;
  onBackToHome: () => void;
  onStopEngine: () => void;
  onArchive: () => void;
  onDelete: () => void;
}

function WorkflowPageInner({
  wsUrl,
  sessionId,
  session,
  engineAlive,
  onStartNewRun,
  onBackToHome,
  onStopEngine,
  onArchive,
  onDelete,
}: WorkflowInnerProps) {
  const {
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
  } = usePipeline(wsUrl, sessionId);
  const [manualSelection, setManualSelection] = useState<string | null>("task");

  const autoSelected = useMemo(() => {
    const running = PIPELINE.find((p) => states[p.id]?.status === "running");
    if (running) return running.id;
    const lastFailed = [...PIPELINE]
      .reverse()
      .find((p) => states[p.id]?.status === "failed");
    if (lastFailed) return lastFailed.id;
    const lastDone = [...PIPELINE]
      .reverse()
      .find((p) => states[p.id]?.status === "passed");
    return lastDone?.id ?? "task";
  }, [states]);

  const selectedId = manualSelection ?? autoSelected;

  const submitTask = (task: SubmittedTask) => {
    const ok = send("task:submit", task);
    if (!ok) {
      // eslint-disable-next-line no-alert
      alert(
        "WebSocket is not connected. The engine isn't running — start `pnpm dev` first."
      );
    }
  };

  const abortRun = () => {
    if (!confirm("Cancel the current pipeline run? The engine will stop.")) return;
    send("pipeline:abort");
  };

  const respondToReview = (
    checkpointId: string,
    payload: {
      decision: "approve" | "edit" | "regenerate";
      editedSpec?: unknown;
      regeneratePrompt?: string;
      notes?: string;
    }
  ) => {
    send(`human:${checkpointId}`, payload);
  };

  const respondToDesignGate = (payload: {
    designRequired: boolean;
    figmaUrl?: string;
    skipReason?: string;
  }) => {
    send("human:design_gate", payload);
  };

  const submitClarifyingAnswers = (payload: {
    answers: Record<string, string>;
    attachments: Record<string, Array<{ name: string; dataUrl: string }>>;
  }) => {
    send("human:product_alignment", payload);
  };

  const toggleAutoMode = () => {
    if (autoMode.enabled) {
      send("auto-mode:set", { enabled: false });
      return;
    }
    const defaultName =
      localStorage.getItem("10xgrace.agentName") || "Riddhi's subagent";
    const name = window.prompt(
      "Name your subagent (this name appears in every auto-mode log):",
      defaultName
    );
    if (!name || !name.trim()) return;
    localStorage.setItem("10xgrace.agentName", name.trim());
    send("auto-mode:set", { enabled: true, agentName: name.trim() });
  };

  return (
    <div
      style={{
        display: "flex",
        height: "100vh",
        background: T.bg,
        color: T.text,
        fontFamily:
          "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
      }}
    >
      <style>{`
        @keyframes spin { to { transform: rotate(360deg) } }
        @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.55} }
        html, body, #root { margin: 0; padding: 0; height: 100%; background: ${T.bg}; }
        * { box-sizing: border-box; }
        ::-webkit-scrollbar { width: 10px; height: 10px; }
        ::-webkit-scrollbar-track { background: ${T.bg}; }
        ::-webkit-scrollbar-thumb { background: ${T.border}; border-radius: 5px; }
        ::-webkit-scrollbar-thumb:hover { background: ${T.borderStrong}; }
      `}</style>
      <Sidebar
        states={states}
        selectedId={selectedId}
        onSelect={setManualSelection}
        runId={runId}
        wsStatus={wsStatus}
      />
      <main
        style={{
          flex: 1,
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
          minHeight: 0,
        }}
      >
        <SessionStrip
          session={session}
          onBackToHome={onBackToHome}
          onStopEngine={onStopEngine}
          onArchive={onArchive}
          onDelete={onDelete}
        />
        <TopBar
          pipelineStatus={pipelineStatus}
          abortReason={abortReason}
          onAbort={abortRun}
          canAbort={pipelineStatus === "running" && wsStatus === "open"}
          engineAlive={engineAlive}
          onStartNewRun={onStartNewRun}
          autoMode={autoMode}
          onToggleAutoMode={toggleAutoMode}
          runsPicker={
            <RunsPicker
              currentRunId={runId}
              runs={savedRuns}
              onRequestList={() => send("runs:list")}
              onResume={(rid, startFrom) =>
                send("runs:resume", { runId: rid, startFrom })
              }
              onNewRun={() => {
                if (
                  pipelineStatus === "running" &&
                  !confirm(
                    "Start a new run? The engine will restart and the current in-flight run will be abandoned (it'll still be in past runs)."
                  )
                ) {
                  return;
                }
                send("runs:new");
                setManualSelection("task");
              }}
            />
          }
        />
        <div style={{ flex: 1, minHeight: 0, overflow: "hidden", display: "flex", flexDirection: "column" }}>
          <CheckpointDetail
            checkpointId={selectedId}
            state={states[selectedId]}
            artifacts={artifacts}
            artifactHistory={artifactHistory}
            onSubmitTask={submitTask}
            onHumanReviewRespond={respondToReview}
            onDesignGateRespond={respondToDesignGate}
            onClarifyingAnswers={submitClarifyingAnswers}
            onRerunStep={(cpId) => {
              if (runId) send("runs:resume", { runId, startFrom: cpId });
            }}
            lastRejection={lastRejection}
            // wsConnected gates the in-form Submit button. Phase 7: also
            // require an attached engine — submitting before the engine is
            // up would have the supervisor drop the message silently.
            wsConnected={wsStatus === "open" && engineAlive}
            runId={runId}
          />
        </div>
        <JourneyBar journey={journey} onSelect={setManualSelection} />
      </main>
      <LogPanel logs={allLogs} />
    </div>
  );
}

// Simplified Grace-style pipeline labels
const CP_SHORT: Record<string, string> = {
  task: "Task",
  l2_gen: "L2",
  l2_review: "L2 review",
  l3_gen: "L3",
  l3_review: "L3 review",
  l4_gen: "L4",
  l4_review: "L4 review",
  implementation: "Impl",
  compiler: "Compiler",
  design_match: "Design match",
  cypress: "Cypress",
  playwright: "Playwright",
  pr_review: "PR review",
  regression: "Regression",
};

function JourneyBar({ journey, onSelect }: { journey: JourneyEvent[]; onSelect: (id: string) => void }) {
  const scrollRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (scrollRef.current) scrollRef.current.scrollLeft = scrollRef.current.scrollWidth;
  }, [journey.length]);

  const visible = journey.filter((e, i) => {
    if (e.kind === "started") {
      const next = journey[i + 1];
      if (next && next.kind === "passed" && next.checkpointId === e.checkpointId) return false;
    }
    return true;
  });

  return (
    <div style={{ borderTop: `1px solid ${T.border}`, background: T.bgElev, height: 38, display: "flex", alignItems: "center" }}>
      <span style={{ padding: "0 10px 0 16px", fontSize: 11, fontWeight: 600, color: T.textSubtle, flexShrink: 0 }}>Journey:</span>
      <div ref={scrollRef} style={{ flex: 1, overflowX: "auto", display: "flex", alignItems: "center", gap: 2, padding: "0 16px 0 0", scrollbarWidth: "none" }}>
        {visible.length === 0
          ? <span style={{ fontSize: 11, color: T.textSubtle, fontStyle: "italic" }}>Pipeline hasn't started</span>
          : visible.map((e, i) => {
            if (e.kind === "rollback") {
              return (
                <div key={i} style={{ display: "flex", alignItems: "center", gap: 2, flexShrink: 0 }}>
                  <span style={{ color: T.textSubtle, fontSize: 12, padding: "0 2px" }}>→</span>
                  <span
                    title={`Retry #${e.attempt} — rolled back to ${CP_SHORT[e.rollbackTo!] ?? e.rollbackTo}`}
                    style={{
                      fontSize: 11, fontWeight: 600, color: T.warn,
                      background: T.warnSoft, borderRadius: 4,
                      padding: "2px 7px", cursor: "default", flexShrink: 0,
                    }}
                  >
                    ↩ retry #{e.attempt}
                  </span>
                </div>
              );
            }

            const isRunning = e.kind === "started";
            const isPassed = e.kind === "passed";
            const fg = isPassed ? T.success : e.kind === "failed" ? T.error : T.accent;
            const bg = isPassed ? T.successSoft : e.kind === "failed" ? T.errorSoft : T.accentSoft;
            const short = CP_SHORT[e.checkpointId] ?? e.checkpointId;

            return (
              <div key={i} style={{ display: "flex", alignItems: "center", gap: 2, flexShrink: 0 }}>
                {i > 0 && <span style={{ color: T.textSubtle, fontSize: 12, padding: "0 2px" }}>→</span>}
                <button
                  onClick={() => onSelect(e.checkpointId)}
                  title={PIPELINE.find(p => p.id === e.checkpointId)?.name}
                  style={{
                    display: "flex", alignItems: "center", gap: 4,
                    padding: "2px 8px", borderRadius: 4,
                    background: bg, border: "none",
                    cursor: "pointer", flexShrink: 0,
                    animation: isRunning ? "pulse 1.4s ease-in-out infinite" : undefined,
                  }}
                >
                  {isRunning && (
                    <div style={{
                      width: 7, height: 7, borderRadius: "50%", flexShrink: 0,
                      border: `1.5px solid ${T.accent}`, borderTopColor: "transparent",
                      animation: "spin 0.8s linear infinite",
                    }} />
                  )}
                  <span style={{ fontSize: 11, fontWeight: 600, color: fg }}>
                    {short}{e.attempt > 0 ? ` #${e.attempt}` : ""}
                  </span>
                  {isPassed && <span style={{ fontSize: 9, color: fg }}>✓</span>}
                  {e.kind === "failed" && <span style={{ fontSize: 9, color: fg }}>✗</span>}
                </button>
              </div>
            );
          })}
      </div>
    </div>
  );
}

function TopBar({
  pipelineStatus,
  abortReason,
  onAbort,
  canAbort,
  engineAlive,
  onStartNewRun,
  autoMode,
  onToggleAutoMode,
  runsPicker,
}: {
  pipelineStatus: "idle" | "running" | "complete" | "aborted";
  abortReason: string | null;
  onAbort: () => void;
  canAbort: boolean;
  /** Phase 7: when false AND pipelineStatus is terminal, swap the
   *  Cancel-run slot for a Start-new-run button. */
  engineAlive: boolean;
  onStartNewRun: () => void;
  autoMode: { enabled: boolean; agentName?: string };
  onToggleAutoMode: () => void;
  runsPicker?: React.ReactNode;
}) {
  let tone: { bg: string; fg: string; label: string; detail?: string } = {
    bg: T.codeBg,
    fg: T.textMuted,
    label: "Idle — submit a task to start",
  };
  if (pipelineStatus === "running") {
    tone = {
      bg: T.accentSoft,
      fg: T.accent,
      label: "Pipeline running",
      detail: "Click any step in the sidebar to inspect its logs and output.",
    };
  } else if (pipelineStatus === "complete") {
    tone = {
      bg: T.successSoft,
      fg: T.success,
      label: "Pipeline complete",
      detail: "All checkpoints passed.",
    };
  } else if (pipelineStatus === "aborted") {
    tone = {
      bg: T.errorSoft,
      fg: T.error,
      label: "Pipeline aborted",
      detail: abortReason ?? "A checkpoint failed after max retries.",
    };
  }

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: 16,
        padding: "14px 32px",
        background: tone.bg,
        borderBottom: `1px solid ${T.border}`,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 14, minWidth: 0 }}>
        <span
          style={{
            width: 8,
            height: 8,
            borderRadius: "50%",
            background: tone.fg,
            flexShrink: 0,
            animation: pipelineStatus === "running" ? "pulse 1.6s ease-in-out infinite" : undefined,
          }}
        />
        <div style={{ minWidth: 0 }}>
          <div style={{ fontWeight: 700, fontSize: 13, color: tone.fg }}>
            {tone.label}
          </div>
          {tone.detail && (
            <div
              style={{
                fontSize: 12,
                color: T.textMuted,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {tone.detail}
            </div>
          )}
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 10, flexShrink: 0 }}>
        <button
          onClick={onToggleAutoMode}
          title={
            autoMode.enabled
              ? `Auto-mode ON — ${autoMode.agentName ?? "unnamed subagent"} is deciding on your behalf`
              : "Click to enable auto-mode and name your subagent"
          }
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            padding: "7px 12px",
            borderRadius: 999,
            border: `1px solid ${autoMode.enabled ? T.success : T.borderStrong}`,
            background: autoMode.enabled ? T.successSoft : T.bgElev,
            color: autoMode.enabled ? T.success : T.text,
            fontWeight: 600,
            fontSize: 12,
            cursor: "pointer",
          }}
        >
          <span
            style={{
              width: 28,
              height: 16,
              borderRadius: 999,
              background: autoMode.enabled ? T.success : T.border,
              position: "relative",
              transition: "background 150ms",
            }}
          >
            <span
              style={{
                position: "absolute",
                top: 2,
                left: autoMode.enabled ? 14 : 2,
                width: 12,
                height: 12,
                borderRadius: "50%",
                background: "#fff",
                transition: "left 150ms",
                boxShadow: "0 1px 2px rgba(0,0,0,0.2)",
              }}
            />
          </span>
          <span>
            {autoMode.enabled
              ? `Auto: ${autoMode.agentName ?? "subagent"}`
              : "Auto mode"}
          </span>
        </button>
        {runsPicker}
        {!engineAlive ? (
          // Phase 7 + 8: any "no live engine" state surfaces a Start CTA.
          // - idle (Phase 8): "Start run" — session has no prior run, user
          //   navigated in without autostart intent.
          // - complete (Phase 7): "Start new run" — pipeline succeeded.
          // - aborted (Phase 7): "Start new run" — pipeline failed; user
          //   wants to retry.
          // Clicking spawns a fresh engine via supervisor.startSession.
          <button
            onClick={onStartNewRun}
            style={{
              padding: "7px 14px",
              borderRadius: 6,
              border: "none",
              background: T.accent,
              color: "#fff",
              fontWeight: 600,
              fontSize: 12,
              cursor: "pointer",
            }}
          >
            {pipelineStatus === "idle" ? "Start run" : "Start new run"}
          </button>
        ) : (
          <button
            onClick={onAbort}
            disabled={!canAbort}
            style={{
              padding: "7px 14px",
              borderRadius: 6,
              border: `1px solid ${canAbort ? T.error : T.border}`,
              background: canAbort ? T.bgElev : "transparent",
              color: canAbort ? T.error : T.textSubtle,
              fontWeight: 600,
              fontSize: 12,
              cursor: canAbort ? "pointer" : "not-allowed",
            }}
          >
            Cancel run
          </button>
        )}
      </div>
    </div>
  );
}

function SessionStrip({
  session,
  onBackToHome,
  onStopEngine,
  onArchive,
  onDelete,
}: {
  session: SessionRecord;
  onBackToHome: () => void;
  onStopEngine: () => void;
  onArchive: () => void;
  onDelete: () => void;
}) {
  const isDefault = session.sessionId === "default";
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "8px 32px",
        background: T.bgSidebar,
        borderBottom: `1px solid ${T.border}`,
        fontSize: 12,
      }}
    >
      <button
        onClick={onBackToHome}
        title="Back to sessions"
        style={{
          padding: "4px 10px",
          borderRadius: 6,
          border: `1px solid ${T.border}`,
          background: T.bgElev,
          color: T.text,
          fontSize: 12,
          cursor: "pointer",
        }}
      >
        ←
      </button>
      <div style={{ display: "flex", flexDirection: "column", minWidth: 0 }}>
        <span style={{ fontWeight: 700, color: T.text, fontSize: 13 }}>
          {session.name}
        </span>
        <span
          style={{
            fontSize: 11,
            color: T.textSubtle,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            maxWidth: 540,
          }}
        >
          {session.projectRoot}
          {session.pid != null ? ` · pid ${session.pid}` : ""}
        </span>
      </div>
      <div style={{ flex: 1 }} />
      <button
        onClick={onStopEngine}
        title="Stop the engine process for this session"
        style={{
          padding: "4px 10px",
          borderRadius: 6,
          border: `1px solid ${T.border}`,
          background: T.bgElev,
          color: T.textMuted,
          fontSize: 11,
          cursor: "pointer",
        }}
      >
        Stop engine
      </button>
      {!isDefault && (
        <>
          <button
            onClick={onArchive}
            style={{
              padding: "4px 10px",
              borderRadius: 6,
              border: `1px solid ${T.border}`,
              background: T.bgElev,
              color: T.textMuted,
              fontSize: 11,
              cursor: "pointer",
            }}
          >
            Archive
          </button>
          <button
            onClick={onDelete}
            style={{
              padding: "4px 10px",
              borderRadius: 6,
              border: `1px solid ${T.error}`,
              background: T.bgElev,
              color: T.error,
              fontSize: 11,
              cursor: "pointer",
            }}
          >
            Delete
          </button>
        </>
      )}
    </div>
  );
}
