import { T } from "../../theme";
import { COMPLEXITY, PRIORITY } from "./enums";
import { Field, Pill, RepeatingList, Section, inputStyle } from "./shared";
import type { WizardAction, WizardState } from "./types";

export function IntegrationStepAcceptance({
  state,
  dispatch,
}: {
  state: WizardState;
  dispatch: (a: WizardAction) => void;
}) {
  const set = (patch: Partial<WizardState>) => dispatch({ type: "set", patch });

  return (
    <div>
      <Section
        title="Acceptance Criteria"
        description="At least 2 required. These become the in-scope checklist for L3 and the validation hook for grpc_test."
      >
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {state.acceptanceCriteria.map((c, i) => (
            <div key={i} style={{ display: "flex", gap: 6 }}>
              <input
                value={c}
                onChange={(e) =>
                  dispatch({ type: "updateCriterion", index: i, text: e.target.value })
                }
                style={{ ...inputStyle, flex: 1 }}
              />
              <button
                type="button"
                onClick={() => dispatch({ type: "removeCriterion", index: i })}
                style={{
                  padding: "0 10px",
                  borderRadius: 6,
                  border: `1px solid ${T.border}`,
                  background: T.bg,
                  color: T.textMuted,
                  cursor: "pointer",
                  fontSize: 13,
                }}
              >
                ✕
              </button>
            </div>
          ))}
          <button
            type="button"
            onClick={() =>
              dispatch({ type: "addCriterion", text: "New criterion" })
            }
            style={{
              padding: "8px 14px",
              borderRadius: 6,
              border: `1px dashed ${T.border}`,
              background: "transparent",
              color: T.textMuted,
              cursor: "pointer",
              fontWeight: 600,
              fontSize: 13,
              alignSelf: "flex-start",
            }}
          >
            + Add criterion
          </button>
        </div>
      </Section>

      <Section title="Prerequisites" description="Optional. Work that must complete before this integration.">
        <RepeatingList
          items={state.prerequisites}
          placeholder="e.g. PCI cert provisioned"
          onAdd={(text) => dispatch({ type: "addPrereq", text })}
          onRemove={(i) => dispatch({ type: "removePrereq", index: i })}
        />
      </Section>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
        <Section title="Estimated Complexity">
          <div style={{ display: "flex", gap: 8 }}>
            {COMPLEXITY.map((c) => (
              <Pill
                key={c}
                active={state.estimatedComplexity === c}
                onClick={() => set({ estimatedComplexity: c })}
              >
                {c}
              </Pill>
            ))}
          </div>
        </Section>
        <Section title="Priority">
          <div style={{ display: "flex", gap: 8 }}>
            {PRIORITY.map((p) => (
              <Pill
                key={p}
                active={state.priority === p}
                onClick={() => set({ priority: p })}
              >
                {p}
              </Pill>
            ))}
          </div>
        </Section>
      </div>

      <Section
        title="Connector Notes"
        description="Optional. Known quirks: idempotency requirements, rate limits, odd field mappings, sandbox gotchas."
      >
        <textarea
          value={state.connectorNotes}
          onChange={(e) => set({ connectorNotes: e.target.value })}
          rows={4}
          placeholder="Requires Idempotency-Key header on writes. Sandbox doesn't echo merchantTxnId. Status field uses 'CAPTURED' not 'CAPTURE'."
          style={{ ...inputStyle, resize: "vertical", fontFamily: "inherit" }}
        />
      </Section>

      <Section
        title="Runner"
        description="claude-code keeps persistent session memory across L3 → implementation → compiler_check → grpc_test fix loops. opencode is stateless."
      >
        <Field label="AI runner" required>
          <div style={{ display: "flex", gap: 12 }}>
            <RunnerCard
              name="Claude Code"
              icon="🧠"
              description="Persistent session across fix loops (Recommended for new connectors)"
              active={state.runner === "claude-code"}
              onClick={() => set({ runner: "claude-code" })}
            />
            <RunnerCard
              name="OpenCode"
              icon="🤖"
              description="Local LLM gateway, stateless per call"
              active={state.runner === "opencode"}
              onClick={() => set({ runner: "opencode" })}
            />
          </div>
        </Field>
        {state.runner && (
          <Field label="Model override (optional)">
            <input
              value={state.runnerModel}
              onChange={(e) => set({ runnerModel: e.target.value })}
              placeholder={
                state.runner === "claude-code" ? "claude-opus-4-7" : "qwen3-coder-480b"
              }
              style={inputStyle}
            />
          </Field>
        )}
      </Section>

      <Section title="Session" description="Where the engine provisions the worktree.">
        <Field label="Source path" required>
          <input
            value={state.sourcePath}
            onChange={(e) => set({ sourcePath: e.target.value })}
            placeholder="/path/to/hyperswitch-prism"
            style={inputStyle}
          />
        </Field>
        <Field label="Copy strategy">
          <div style={{ display: "flex", gap: 8 }}>
            {(
              [
                ["git-worktree", "Git worktree (fast)"],
                ["full", "Full copy"],
                ["shallow", "Shallow clone"],
              ] as const
            ).map(([v, label]) => (
              <Pill
                key={v}
                active={state.copyStrategy === v}
                onClick={() => set({ copyStrategy: v })}
              >
                {label}
              </Pill>
            ))}
          </div>
        </Field>
      </Section>
    </div>
  );
}

function RunnerCard({
  name,
  icon,
  description,
  active,
  onClick,
}: {
  name: string;
  icon: string;
  description: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        flex: 1,
        padding: "14px 18px",
        borderRadius: 8,
        border: `2px solid ${active ? T.accent : T.border}`,
        background: active ? T.accentSoft : T.bg,
        cursor: "pointer",
        textAlign: "left",
      }}
    >
      <div style={{ fontSize: 22, marginBottom: 6 }}>{icon}</div>
      <div
        style={{
          fontWeight: 700,
          fontSize: 14,
          color: active ? T.accent : T.text,
          marginBottom: 3,
        }}
      >
        {name}
      </div>
      <div style={{ fontSize: 12, color: T.textMuted, lineHeight: 1.4 }}>
        {description}
      </div>
    </button>
  );
}
