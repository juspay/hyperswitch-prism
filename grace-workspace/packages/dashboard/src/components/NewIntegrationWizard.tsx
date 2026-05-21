import { useMemo, useReducer, useState } from "react";
import { T } from "../theme";
import type { SessionWithTaskInput } from "./UnifiedCreateSessionModal";
import { IntegrationStepAcceptance } from "./wizard/IntegrationStepAcceptance";
import { IntegrationStepBasics } from "./wizard/IntegrationStepBasics";
import { IntegrationStepReview } from "./wizard/IntegrationStepReview";
import { IntegrationStepTechnical } from "./wizard/IntegrationStepTechnical";
import { Stepper } from "./wizard/Stepper";
import {
  buildSessionWithTaskInput,
  canAdvanceStep,
  canSubmit,
  makeInitialState,
  wizardReducer,
} from "./wizard/types";

interface Props {
  defaultSourcePath: string;
  wsConnected: boolean;
  onSubmit: (input: SessionWithTaskInput) => Promise<void> | void;
  onClose: () => void;
}

/**
 * Multi-step wizard for adding a brand-new connector to prism.
 * - Step 1: Basics (name, description, high-quality doc URLs)
 * - Step 2: Technical (auth, currency unit, flows, payment methods)
 * - Step 3: Acceptance (criteria, runner, session)
 * - Step 4: Review & launch
 *
 * Produces a SessionWithTaskInput identical in shape to what
 * UnifiedCreateSessionModal produces, with the wizard's structured fields
 * added on. Submission reuses Homepage's existing onCreateAndStart handler.
 */
export function NewIntegrationWizard({
  defaultSourcePath,
  wsConnected,
  onSubmit,
  onClose,
}: Props) {
  const [state, dispatch] = useReducer(
    wizardReducer,
    defaultSourcePath,
    makeInitialState,
  );
  const [submitting, setSubmitting] = useState(false);

  const reached = useMemo(() => {
    let r = 1;
    for (let s = 1; s <= 3; s++) {
      if (canAdvanceStep({ ...state, step: s })) r = s + 1;
      else break;
    }
    return r;
  }, [state]);

  const stepValid = canAdvanceStep(state);
  const submitReady = canSubmit(state) && wsConnected && !submitting;

  const handleLaunch = async () => {
    if (!submitReady) return;
    setSubmitting(true);
    try {
      await onSubmit(buildSessionWithTaskInput(state));
      onClose();
    } finally {
      setSubmitting(false);
    }
  };

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
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 820,
          maxHeight: "92vh",
          background: T.bgElev,
          borderRadius: 10,
          boxShadow: T.shadowLg,
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
            <IntegrationStepBasics state={state} dispatch={dispatch} />
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
            Cancel
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
                    Launching…
                  </>
                ) : (
                  <>Launch Integration →</>
                )}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
