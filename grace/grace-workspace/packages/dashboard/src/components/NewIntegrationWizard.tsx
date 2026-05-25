import { useEffect, useMemo, useReducer, useState } from "react";
import { T } from "../theme";
import type { SessionWithTaskInput, TaskDefinition } from "./UnifiedCreateSessionModal";
import { IntegrationStepAcceptance } from "./wizard/IntegrationStepAcceptance";
import { IntegrationStepBasics } from "./wizard/IntegrationStepBasics";
import { IntegrationStepReview } from "./wizard/IntegrationStepReview";
import { IntegrationStepTechnical } from "./wizard/IntegrationStepTechnical";
import { Stepper } from "./wizard/Stepper";
import {
  buildSessionWithTaskInput,
  canAdvanceStep,
  canSubmit,
  hydrateWizardState,
  makeInitialState,
  serializeWizardState,
  wizardReducer,
} from "./wizard/types";
import type { WizardState } from "./wizard/types";

type SessionModeProps = {
  mode?: "session";
  onSubmit: (input: SessionWithTaskInput) => Promise<void> | void;
  defaultSourcePath: string;
};

type TaskModeProps = {
  mode: "task";
  onSubmitTask: (task: TaskDefinition) => Promise<void> | void;
  /** Persist the wizard draft under `grace:wizard-draft:<sessionId>`. */
  sessionId: string;
  /** Not used in task mode, but required by the reducer's initial state. */
  defaultSourcePath?: string;
};

type CommonProps = {
  wsConnected: boolean;
  onClose: () => void;
  /** When true, drops the fixed-overlay modal wrapper and renders inline. */
  embedded?: boolean;
};

type Props = CommonProps & (SessionModeProps | TaskModeProps);

/**
 * Multi-step wizard for adding a brand-new connector to prism.
 * - Step 1: Basics (name, description, high-quality doc URLs)
 * - Step 2: Technical (auth, currency unit, flows, payment methods)
 * - Step 3: Acceptance (criteria, runner, session)
 * - Step 4: Review & launch
 *
 * Two submit modes:
 * - "session" (default): builds a SessionWithTaskInput; the caller creates a
 *   new session AND starts the engine. Used by the Homepage tile.
 * - "task": builds just the TaskDefinition for an already-open session.
 *   Used from inside the Task Definition checkpoint.
 *
 * Two render modes:
 * - Default: full-screen modal with backdrop (Homepage entry).
 * - `embedded`: inline panel, no overlay (Task Definition entry).
 */
export function NewIntegrationWizard(props: Props) {
  const { wsConnected, onClose, embedded } = props;
  const mode = props.mode ?? "session";
  const defaultSourcePath =
    "defaultSourcePath" in props && props.defaultSourcePath
      ? props.defaultSourcePath
      : "";
  const draftKey =
    mode === "task" ? `grace:wizard-draft:${(props as TaskModeProps).sessionId}` : null;

  const [state, dispatch] = useReducer(
    wizardReducer,
    defaultSourcePath,
    (sourcePath: string) => {
      if (draftKey) {
        try {
          const raw = localStorage.getItem(draftKey);
          if (raw) {
            const hydrated = hydrateWizardState(JSON.parse(raw), sourcePath);
            if (hydrated) return hydrated;
          }
        } catch {
          /* corrupt draft — fall through to fresh state */
        }
      }
      return makeInitialState(sourcePath);
    },
  );
  const [submitting, setSubmitting] = useState(false);

  // Persist after every dispatch (debounced).
  useEffect(() => {
    if (!draftKey) return;
    const t = setTimeout(() => {
      try {
        localStorage.setItem(draftKey, JSON.stringify(serializeWizardState(state)));
      } catch {
        /* quota / disabled — ignore */
      }
    }, 300);
    return () => clearTimeout(t);
  }, [draftKey, state]);

  const reached = useMemo(() => {
    let r = 1;
    for (let s = 1; s <= 3; s++) {
      if (canAdvanceStep({ ...state, step: s })) r = s + 1;
      else break;
    }
    return r;
  }, [state]);

  const stepValid = canAdvanceStep(state);
  // In `task` mode the wizard runs inside an already-created session, so
  // `state.sourcePath` is irrelevant (the engine has the session's
  // projectRoot from session metadata). `canSubmit` includes a sourcePath
  // check that's needed for the Homepage / "session" mode but blocks the
  // embedded Task-Definition flow because we never pass defaultSourcePath
  // down. Sub the check out for an embedded-mode-friendly variant.
  const canSubmitEmbedded = (s: WizardState): boolean =>
    canAdvanceStep({ ...s, step: 1 }) &&
    canAdvanceStep({ ...s, step: 2 }) &&
    canAdvanceStep({ ...s, step: 3 }) &&
    s.discoveryStatus !== "running";
  const submitReady =
    (mode === "task" ? canSubmitEmbedded(state) : canSubmit(state)) &&
    wsConnected &&
    !submitting;

  const handleLaunch = async () => {
    if (!submitReady) return;
    setSubmitting(true);
    try {
      const built = buildSessionWithTaskInput(state);
      if (mode === "task") {
        if (built.initialTask) {
          await (props as TaskModeProps).onSubmitTask(built.initialTask);
        }
      } else {
        await (props as SessionModeProps).onSubmit(built);
      }
      // Submit succeeded — drop the persisted draft so the next mount of
      // this session starts fresh instead of resurrecting the just-submitted
      // form. Failure paths intentionally leave the draft so the user can
      // retry without re-typing.
      if (draftKey) {
        try { localStorage.removeItem(draftKey); } catch { /* ignore */ }
      }
      onClose();
    } finally {
      setSubmitting(false);
    }
  };

  const launchLabel =
    mode === "task" ? "Submit task →" : "Launch Integration →";
  const launchingLabel = mode === "task" ? "Submitting…" : "Launching…";

  const panel = (
    <div
      onClick={(e) => e.stopPropagation()}
      style={{
        width: embedded ? "100%" : 820,
        maxWidth: embedded ? 820 : undefined,
        maxHeight: embedded ? undefined : "92vh",
        background: T.bgElev,
        borderRadius: embedded ? 12 : 10,
        boxShadow: embedded ? T.shadow : T.shadowLg,
        border: embedded ? `1px solid ${T.border}` : "none",
        display: "flex",
        flexDirection: "column",
        color: T.text,
        overflow: "hidden",
      }}
    >
      {/* Header */}
      <div
        style={{
          padding: "20px 24px",
          borderBottom: `1px solid ${T.border}`,
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          background: T.bgSidebar,
        }}
      >
        <div>
          <h2 style={{ margin: 0, fontSize: 18, fontWeight: 700 }}>
            Integrate a New Connector
          </h2>
          <p style={{ margin: "4px 0 0", fontSize: 12, color: T.textMuted }}>
            Collects structured fields so L2/L3 planning and codegen don't
            have to guess.
          </p>
        </div>
        {!embedded && (
          <button
            onClick={onClose}
            style={{
              border: "none",
              background: "transparent",
              cursor: "pointer",
              color: T.textMuted,
              fontSize: 24,
              lineHeight: 1,
            }}
            aria-label="Close"
          >
            ×
          </button>
        )}
      </div>

      {/* Stepper */}
      <Stepper
        current={state.step}
        reached={reached}
        onJump={(n) => dispatch({ type: "goto", step: n })}
      />

      {/* Content */}
      <div
        style={{
          flex: 1,
          overflowY: "auto",
          padding: 24,
        }}
      >
        {!wsConnected && (
          <div
            style={{
              background: T.warnSoft,
              color: T.warn,
              padding: "10px 14px",
              borderRadius: 6,
              fontSize: 13,
              marginBottom: 16,
              border: `1px solid ${T.warn}`,
            }}
          >
            Supervisor disconnected — connect before launching.
          </div>
        )}

        {state.step === 1 && (
          <IntegrationStepBasics
            state={state}
            dispatch={dispatch}
            sessionId={mode === "task" ? (props as TaskModeProps).sessionId : undefined}
          />
        )}
        {state.step === 2 && (
          <IntegrationStepTechnical state={state} dispatch={dispatch} />
        )}
        {state.step === 3 && (
          <IntegrationStepAcceptance state={state} dispatch={dispatch} />
        )}
        {state.step === 4 && (
          <IntegrationStepReview state={state} dispatch={dispatch} />
        )}
      </div>

      {/* Footer */}
      <div
        style={{
          padding: "16px 24px",
          borderTop: `1px solid ${T.border}`,
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          background: T.bgSidebar,
          gap: 12,
        }}
      >
        <button
          onClick={onClose}
          disabled={submitting}
          style={{
            padding: "10px 18px",
            borderRadius: 6,
            border: `1px solid ${T.border}`,
            background: "transparent",
            color: T.text,
            fontSize: 13,
            cursor: submitting ? "not-allowed" : "pointer",
            fontWeight: 500,
          }}
        >
          {embedded ? "Back to standard task" : "Cancel"}
        </button>
        <div style={{ display: "flex", gap: 10 }}>
          {state.step > 1 && (
            <button
              onClick={() => dispatch({ type: "back" })}
              disabled={submitting}
              style={{
                padding: "10px 18px",
                borderRadius: 6,
                border: `1px solid ${T.border}`,
                background: T.bg,
                color: T.text,
                fontSize: 13,
                cursor: "pointer",
                fontWeight: 500,
              }}
            >
              ← Back
            </button>
          )}
          {state.step < 4 && (
            <button
              onClick={() => dispatch({ type: "next" })}
              disabled={!stepValid}
              style={{
                padding: "10px 22px",
                borderRadius: 6,
                border: "none",
                background: stepValid ? T.accent : T.border,
                color: "#fff",
                fontSize: 13,
                fontWeight: 600,
                cursor: stepValid ? "pointer" : "not-allowed",
              }}
            >
              Next →
            </button>
          )}
          {state.step === 4 && (
            <button
              onClick={handleLaunch}
              disabled={!submitReady}
              style={{
                padding: "10px 26px",
                borderRadius: 6,
                border: "none",
                background: submitReady ? T.accent : T.border,
                color: "#fff",
                fontSize: 13,
                fontWeight: 700,
                cursor: submitReady ? "pointer" : "not-allowed",
                display: "flex",
                alignItems: "center",
                gap: 8,
              }}
            >
              {submitting ? (
                <>
                  <span style={{ animation: "spin 1s linear infinite" }}>⟳</span>
                  {launchingLabel}
                </>
              ) : (
                <>{launchLabel}</>
              )}
            </button>
          )}
        </div>
      </div>
    </div>
  );

  if (embedded) return panel;

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.32)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 100,
        padding: 20,
      }}
      onClick={onClose}
    >
      {panel}
    </div>
  );
}
