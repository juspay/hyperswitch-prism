import React, { useState, useEffect } from "react";
import type { CheckpointState, AttemptRecord } from "../hooks/usePipeline";
import { PIPELINE } from "../hooks/usePipeline";
import { TaskForm, type SubmittedTask } from "./TaskForm";
import { ArtifactView } from "./ArtifactView";
import { HumanReview } from "./HumanReview";
import { DesignGatePrompt } from "./DesignGatePrompt";
import { ClarifyingQuestions } from "./ClarifyingQuestions";
import { LoadingState } from "./LoadingState";
import { RetryHistory, type RetryAttempt } from "./RetryHistory";
import { T } from "../theme";

const STATUS_BADGE: Record<string, { bg: string; fg: string; dot: string; label: string }> = {
  idle: { bg: T.codeBg, fg: T.textMuted, dot: T.textMuted, label: "Idle" },
  running: { bg: T.accentSoft, fg: T.accent, dot: T.accent, label: "Running" },
  passed: { bg: T.successSoft, fg: T.success, dot: T.success, label: "Passed" },
  failed: { bg: T.errorSoft, fg: T.error, dot: T.error, label: "Failed" },
  skipped: { bg: T.warnSoft, fg: T.warn, dot: T.warn, label: "Skipped" },
};

function StatusBadge({ status }: { status: string }) {
  const s = STATUS_BADGE[status] ?? STATUS_BADGE.idle!;
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 7,
        background: s.bg,
        color: s.fg,
        padding: "5px 12px 5px 10px",
        borderRadius: 999,
        fontSize: 11,
        fontWeight: 700,
        letterSpacing: 0.3,
      }}
    >
      <span
        style={{
          width: 7,
          height: 7,
          borderRadius: "50%",
          background: s.dot,
          animation: status === "running" ? "pulse 1.6s ease-in-out infinite" : undefined,
        }}
      />
      {s.label}
    </span>
  );
}

export function CheckpointDetail({
  sessionId,
  checkpointId,
  state,
  artifacts,
  artifactHistory,
  logsByCp,
  onSubmitTask,
  onHumanReviewRespond,
  onDesignGateRespond,
  onClarifyingAnswers,
  onRerunStep,
  lastRejection,
  wsConnected,
  runId,
  onTaskKindChange,
}: {
  sessionId: string;
  checkpointId: string;
  state: CheckpointState | undefined;
  artifacts: Record<string, unknown>;
  artifactHistory?: Record<string, Record<number, AttemptRecord>>;
  /**
   * Per-checkpoint log lines from the WS bus. Used to feed the
   * `LoadingState` spinner's live-log tail so the user sees what the
   * agent is doing during long-running checkpoints (implementation,
   * per-flow subagents, scaffold) instead of just a spinner. Optional
   * for backward compat with callers that don't have logsByCp.
   */
  logsByCp?: Record<string, Array<{ msg: string; level: string }>>;
  onSubmitTask: (task: SubmittedTask) => void;
  onHumanReviewRespond: (
    checkpointId: string,
    payload: {
      decision: "approve" | "edit" | "regenerate";
      editedSpec?: unknown;
      regeneratePrompt?: string;
      notes?: string;
    }
  ) => void;
  onDesignGateRespond: (payload: {
    designRequired: boolean;
    figmaUrl?: string;
    skipReason?: string;
  }) => void;
  onClarifyingAnswers: (payload: {
    answers: Record<string, string>;
    attachments: Record<string, Array<{ name: string; dataUrl: string }>>;
  }) => void;
  onRerunStep: (checkpointId: string) => void;
  lastRejection: { checkpointId: string; reason: string; ts: string } | null;
  wsConnected: boolean;
  runId: string | undefined;
  /**
   * Forwarded to the embedded `TaskForm`. Lets the sidebar overlay
   * skipped status on rows belonging only to the OTHER workflow before
   * the engine runs (see WorkflowPage previewTaskKind state).
   */
  onTaskKindChange?: (kind: "standard" | "integrate") => void;
}) {
  const meta = PIPELINE.find((p) => p.id === checkpointId);
  if (!meta || !state) {
    return <div style={{ color: T.textMuted, padding: 24 }}>Unknown checkpoint.</div>;
  }

  const artifactKey: Record<string, string> = {
    task: "task",
    product_alignment: "productAlignment",
    feature_research: "featureResearch",
    design_gate: "designGate",
    l2_planning: "l2",
    l2_review: "l2Review",
    l3_analysis: "l3",
    l3_review: "l3Review",
    implementation: "implementation",
    compiler: "compilationErrors",
    compiler_check: "compilerCheck",
    grpc_test: "grpcTest",
    design_match: "designDiff",
    cypress: "cypressReport",
    playwright: "playwrightReport",
    pr_review: "prReview",
    regression: "regression",
  };
  const aKey = artifactKey[checkpointId];
  const artifact = aKey ? artifacts[aKey] : undefined;

  // Retry history tracking - use global artifactHistory if available
  // Display is 1-based (Attempt 1, Attempt 2), internal storage is 0-based
  const [selectedRetryAttempt, setSelectedRetryAttempt] = useState<number>(state.retries + 1);

  // Reset selected attempt when checkpoint changes or retries change externally
  useEffect(() => {
    setSelectedRetryAttempt(state.retries + 1); // Set to 1-based current attempt
  }, [checkpointId, state.retries]);

  // Get artifact history for this checkpoint from global state
  const checkpointHistory = artifactHistory?.[checkpointId] ?? {};

  // Build list of retry attempts for the dropdown (1-based display)
  // Internal state uses 0-based, UI displays 1-based (Attempt 1, Attempt 2, etc.)
  const retryAttempts: RetryAttempt[] = (() => {
    const attempts: RetryAttempt[] = [];
    const currentRetry = state.retries; // 0 = first attempt, 1 = first retry, etc.

    // Add ALL historical attempts (Attempt 1 to Attempt currentRetry)
    for (let i = 0; i < currentRetry; i++) {
      attempts.push({
        attempt: i + 1, // Display as 1-based
        status: "failed",
        timestamp: new Date(Date.now() - (currentRetry - i) * 60000).toISOString(),
      });
    }

    // Add current attempt (1-based: currentRetry + 1)
    attempts.push({
      attempt: currentRetry + 1,
      status: state.status === "running" ? "running" : state.status === "passed" ? "passed" : "failed",
      timestamp: new Date().toISOString(),
    });

    return attempts;
  })();

  // Get the artifact to display based on selected retry attempt
  // Convert 1-based display selection to 0-based internal lookup
  const internalAttemptIndex = selectedRetryAttempt - 1;
  const isCurrentAttempt = selectedRetryAttempt === state.retries + 1;
  const priorAttempt: AttemptRecord | undefined = isCurrentAttempt
    ? undefined
    : checkpointHistory[internalAttemptIndex];
  // For prior attempts, an empty `{}` artifacts record means "no artifact
  // payload" — we want to render the placeholder, not a blank Result.
  const priorHasArtifacts =
    priorAttempt &&
    priorAttempt.artifacts &&
    Object.keys(priorAttempt.artifacts).length > 0;
  const displayArtifact = isCurrentAttempt
    ? artifact
    : priorHasArtifacts
      ? priorAttempt!.artifacts
      : undefined;

  const isTaskStep = checkpointId === "task";
  const isDesignGate = checkpointId === "design_gate";
  const isProductAlignment = checkpointId === "product_alignment";
  // The supervisor pre-enqueues a placeholder run (empty title) before the
  // child engine boots, so `artifacts.task` can be a truthy object even
  // before the user has submitted anything. Mirror the engine's heuristic
  // at packages/core/src/checkpoints/task.ts:26 — a real task has a
  // non-empty title — so the dashboard and engine agree on what "submitted"
  // means.
  const taskAlreadySubmitted =
    !!(artifacts.task as { title?: string } | undefined)?.title?.trim();

  const pmWaiting = Boolean(
    isProductAlignment &&
      state.waiting &&
      Array.isArray((state.waiting.spec as any)?.questions)
  );

  // Treat the partial "pendingQuestions" artifact as not-yet-a-real-result,
  // so the loading card comes back while the PM re-runs with the answers.
  const artifactIsPartial =
    checkpointId === "product_alignment" &&
    artifact &&
    typeof artifact === "object" &&
    (artifact as any).pendingQuestions !== undefined;

  const showLoading = Boolean(
    state.status === "running" &&
      !state.waiting &&
      (artifact === undefined || artifactIsPartial)
  );
  const showGenericReview = Boolean(
    state.waiting && !isTaskStep && !isDesignGate && !pmWaiting
  );

  const globalIdx = PIPELINE.findIndex((p) => p.id === meta.id);

  return (
    <div style={{ padding: "28px 36px 36px", flex: 1, overflowY: "auto", minHeight: 0 }}>
      {/* Eyebrow */}
      <div
        style={{
          fontSize: 11,
          color: T.textSubtle,
          fontWeight: 600,
          textTransform: "uppercase",
          letterSpacing: 1.2,
          marginBottom: 8,
        }}
      >
        Step {String(globalIdx + 1).padStart(2, "0")} ·{" "}
        {meta.type === "human" ? "Human gate" : "Automated"}
      </div>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 14,
          marginBottom: 28,
          flexWrap: "wrap",
        }}
      >
        <h1
          style={{
            margin: 0,
            fontSize: 26,
            fontWeight: 700,
            color: T.text,
            letterSpacing: -0.3,
          }}
        >
          {meta.name}
        </h1>
        <StatusBadge status={state.status} />
        {state.retries > 0 && (
          <span
            style={{
              color: T.warn,
              fontSize: 11,
              fontWeight: 600,
              padding: "4px 10px",
              borderRadius: 999,
              background: T.warnSoft,
              textTransform: "uppercase",
              letterSpacing: 0.5,
            }}
          >
            retry {state.retries}
          </span>
        )}
        {runId && !isTaskStep && (
          <button
            onClick={() => {
              const msg =
                state.status === "running"
                  ? `Re-run "${meta.name}"? The current in-flight execution will be abandoned.`
                  : state.status === "passed"
                    ? `Re-run "${meta.name}"? Downstream stages will be reset.`
                    : `Re-run "${meta.name}"?`;
              if (window.confirm(msg)) onRerunStep(checkpointId);
            }}
            disabled={!wsConnected}
            title={
              wsConnected
                ? `Restart this run at "${meta.name}"`
                : "Engine is offline — cannot re-run"
            }
            style={{
              fontSize: 11,
              fontWeight: 600,
              padding: "5px 12px",
              borderRadius: 6,
              border: `1px solid ${T.accent}`,
              background: "transparent",
              color: T.accent,
              cursor: wsConnected ? "pointer" : "not-allowed",
              opacity: wsConnected ? 1 : 0.5,
              marginLeft: "auto",
            }}
          >
            ↻ Re-run this step
          </button>
        )}
      </div>

      {/* Retry History Selector - shown when there are retries */}
      {(state.retries > 0 || Object.keys(checkpointHistory).length > 0) && (
        <section style={{ marginBottom: 16 }}>
          <RetryHistory
            currentAttempt={state.retries + 1}  // 1-based
            attempts={retryAttempts}
            selectedAttempt={selectedRetryAttempt}
            onSelectAttempt={setSelectedRetryAttempt}
            onBackToCurrent={() => setSelectedRetryAttempt(state.retries + 1)}  // 1-based
          />
        </section>
      )}

      {isTaskStep && !taskAlreadySubmitted && (
        <section style={{ marginBottom: 32 }}>
          <SectionTitle>Submit task</SectionTitle>
          <div
            style={{
              color: T.textMuted,
              fontSize: 13,
              marginBottom: 12,
              maxWidth: 560,
            }}
          >
            Fill in the task below and hit submit. The pipeline will pick it up and
            advance to product alignment.
          </div>
          <TaskForm
            sessionId={sessionId}
            onSubmit={onSubmitTask}
            wsConnected={wsConnected}
            onTaskKindChange={onTaskKindChange}
          />
        </section>
      )}

      {isTaskStep && taskAlreadySubmitted && (
        <SubmittedTaskView task={artifacts.task as Record<string, unknown>} />
      )}

      {state.waiting != null && isDesignGate ? (
        <section style={{ marginBottom: 32 }}>
          <SectionTitle>Design gate</SectionTitle>
          <DesignGatePrompt
            currentFigmaUrl={(state.waiting.spec as any)?.currentFigmaUrl}
            onRespond={onDesignGateRespond}
          />
        </section>
      ) : null}

      {pmWaiting && state.waiting != null ? (
        <section style={{ marginBottom: 32 }}>
          <SectionTitle>Clarifications needed</SectionTitle>
          <ClarifyingQuestions
            notes={(state.waiting.spec as any)?.notes}
            questions={(state.waiting.spec as any)?.questions ?? []}
            onSubmit={onClarifyingAnswers}
          />
        </section>
      ) : null}

      {showGenericReview && state.waiting ? (
        <section style={{ marginBottom: 32 }}>
          <SectionTitle>Review</SectionTitle>
          <HumanReview
            checkpointId={checkpointId}
            spec={state.waiting.spec}
            onRespond={(payload) => onHumanReviewRespond(checkpointId, payload)}
            rejectionReason={
              lastRejection && lastRejection.checkpointId === checkpointId
                ? lastRejection.reason
                : null
            }
          />
        </section>
      ) : null}

      {showLoading && (
        <section style={{ marginBottom: 32 }}>
          <LoadingState
            checkpointId={checkpointId}
            logs={
              // Last ~15 lines, msg only. Empty array = LoadingState
              // renders just the spinner + jokes (no log box).
              logsByCp
                ? (logsByCp[checkpointId] ?? []).slice(-15).map((l) => l.msg)
                : undefined
            }
          />
        </section>
      )}

      {displayArtifact !== undefined && !artifactIsPartial && (
        <section style={{ marginBottom: 32 }}>
          <SectionTitle>Result</SectionTitle>
          <ArtifactView checkpointId={checkpointId} artifact={displayArtifact} artifacts={artifacts} isRunning={state.status === "running"} />
        </section>
      )}

      {/* Empty-state placeholder for past attempts that produced no artifact
          payload (e.g. checkpoints whose outer timeout fired before they
          could return artifacts). Shows whatever metadata we DID capture —
          status, errors, output — instead of a silently blank pane. */}
      {!isCurrentAttempt && !priorHasArtifacts && (
        <section style={{ marginBottom: 32 }}>
          <SectionTitle>Attempt {selectedRetryAttempt}</SectionTitle>
          <div
            style={{
              padding: "20px 22px",
              border: `1px dashed ${T.border}`,
              borderRadius: 10,
              fontSize: 13,
              color: T.textMuted,
              background: T.codeBg,
              maxWidth: 720,
            }}
          >
            <div style={{ marginBottom: 10 }}>
              <strong style={{ color: priorAttempt?.status === "failed" ? T.error : T.textMuted }}>
                {priorAttempt?.status === "failed" ? "Failed" : "No artifact captured"}
              </strong>
              {priorAttempt === undefined && (
                <span> — no record found for this attempt.</span>
              )}
            </div>
            {priorAttempt?.errors && priorAttempt.errors.length > 0 && (
              <div style={{ marginBottom: 10 }}>
                <div style={{ fontWeight: 600, color: T.text, marginBottom: 4 }}>
                  Errors
                </div>
                <pre
                  style={{
                    margin: 0,
                    padding: "8px 10px",
                    background: T.errorSoft,
                    color: T.error,
                    borderRadius: 6,
                    fontSize: 12,
                    whiteSpace: "pre-wrap",
                    wordBreak: "break-word",
                  }}
                >
                  {priorAttempt.errors.join("\n")}
                </pre>
              </div>
            )}
            {priorAttempt?.output && (
              <div>
                <div style={{ fontWeight: 600, color: T.text, marginBottom: 4 }}>
                  Output
                </div>
                <pre
                  style={{
                    margin: 0,
                    padding: "8px 10px",
                    background: T.codeBg,
                    color: T.text,
                    borderRadius: 6,
                    fontSize: 12,
                    maxHeight: 280,
                    overflow: "auto",
                    whiteSpace: "pre-wrap",
                    wordBreak: "break-word",
                  }}
                >
                  {priorAttempt.output.slice(0, 4000)}
                </pre>
              </div>
            )}
          </div>
        </section>
      )}

      {artifact === undefined &&
        !isTaskStep &&
        !isDesignGate &&
        !state.waiting &&
        state.status === "idle" && (
          <div
            style={{
              padding: "32px 20px",
              textAlign: "center",
              color: T.textSubtle,
              fontSize: 13,
              border: `1px dashed ${T.border}`,
              borderRadius: 10,
              maxWidth: 560,
            }}
          >
            This step hasn't run yet.
          </div>
        )}
    </div>
  );
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <h2
      style={{
        fontSize: 11,
        fontWeight: 700,
        color: T.textMuted,
        textTransform: "uppercase",
        letterSpacing: 0.8,
        margin: "0 0 10px 0",
      }}
    >
      {children}
    </h2>
  );
}

/**
 * Rendered for the `task` checkpoint AFTER a task is submitted. Surfaces the
 * captured wizard fields — most importantly `techSpecMarkdown`, which is
 * otherwise only ever visible in the in-memory wizard state and gets lost
 * once the user closes the wizard / navigates back from the workflow view.
 */
function SubmittedTaskView({ task }: { task: Record<string, unknown> }) {
  const [specOpen, setSpecOpen] = useState(true);
  const get = <T,>(k: string): T | undefined => task[k] as T | undefined;
  const title = get<string>("title") ?? "(no title)";
  const description = get<string>("description") ?? "";
  const workflowType = get<string>("workflowType");
  const targets = (get<string[]>("targetConnectors") ?? []).join(", ");
  const baseUrl = get<string>("baseUrl");
  const sandboxUrl = get<string>("sandboxUrl");
  const flows = (get<string[]>("flows") ?? get<string[]>("supportedFlows") ?? []).join(", ");
  const authScheme = get<string>("authScheme");
  const currencyUnit = get<string>("currencyUnit");
  const paymentMethod = get<string>("paymentMethod");
  const techSpec = get<string>("techSpecMarkdown") ?? "";
  const discoveredUrls = get<string[]>("discoveredConnectorUrls") ?? [];

  const fields: Array<[string, string | undefined]> = [
    ["Workflow", workflowType],
    ["Connector(s)", targets || undefined],
    ["Payment method", paymentMethod || undefined],
    ["Flows", flows || undefined],
    ["Auth scheme", authScheme],
    ["Currency unit", currencyUnit],
    ["Base URL", baseUrl],
    ["Sandbox URL", sandboxUrl],
  ].filter(([, v]) => !!v) as Array<[string, string | undefined]>;

  return (
    <section style={{ marginBottom: 32 }}>
      <SectionTitle>Submitted task</SectionTitle>
      <div
        style={{
          border: `1px solid ${T.border}`,
          borderRadius: 10,
          padding: "18px 20px",
          background: T.bgElev,
          maxWidth: 760,
        }}
      >
        <div style={{ fontSize: 15, fontWeight: 700, color: T.text, marginBottom: 6 }}>
          {title}
        </div>
        {description && (
          <div style={{ fontSize: 13, color: T.textMuted, marginBottom: 14 }}>
            {description}
          </div>
        )}
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "auto 1fr",
            columnGap: 14,
            rowGap: 6,
            fontSize: 12.5,
            marginBottom: 14,
          }}
        >
          {fields.map(([k, v]) => (
            <React.Fragment key={k}>
              <div style={{ color: T.textSubtle, fontWeight: 600 }}>{k}</div>
              <div style={{ color: T.text }}>{v}</div>
            </React.Fragment>
          ))}
        </div>

        {discoveredUrls.length > 0 && (
          <div style={{ marginBottom: 14 }}>
            <div
              style={{
                fontSize: 11,
                fontWeight: 700,
                color: T.textSubtle,
                textTransform: "uppercase",
                letterSpacing: 0.6,
                marginBottom: 6,
              }}
            >
              Discovered docs ({discoveredUrls.length})
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
              {discoveredUrls.map((u, i) => (
                <a
                  key={i}
                  href={u}
                  target="_blank"
                  rel="noreferrer"
                  style={{
                    fontSize: 12,
                    color: T.accent,
                    textDecoration: "none",
                    wordBreak: "break-all",
                  }}
                >
                  {u}
                </a>
              ))}
            </div>
          </div>
        )}

        {techSpec.trim().length > 0 && (
          <div>
            <button
              onClick={() => setSpecOpen((o) => !o)}
              style={{
                background: "transparent",
                border: "none",
                color: T.accent,
                fontSize: 12,
                fontWeight: 700,
                padding: 0,
                cursor: "pointer",
                marginBottom: 8,
                letterSpacing: 0.6,
                textTransform: "uppercase",
              }}
            >
              {specOpen ? "▼" : "▶"} Tech spec ({techSpec.length.toLocaleString()} chars)
            </button>
            {specOpen && (
              <pre
                style={{
                  margin: 0,
                  padding: "12px 14px",
                  background: T.codeBg,
                  border: `1px solid ${T.border}`,
                  borderRadius: 8,
                  fontSize: 12,
                  lineHeight: 1.5,
                  maxHeight: 500,
                  overflow: "auto",
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-word",
                  color: T.text,
                  fontFamily:
                    "ui-monospace, SFMono-Regular, Menlo, Monaco, 'Cascadia Code', monospace",
                }}
              >
                {techSpec}
              </pre>
            )}
          </div>
        )}
      </div>
    </section>
  );
}
