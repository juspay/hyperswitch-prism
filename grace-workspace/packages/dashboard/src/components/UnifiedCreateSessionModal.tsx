import { useState, useMemo } from "react";
import { T } from "../theme";
import type { SessionCopyStrategy } from "../hooks/useSessions";

export interface TaskDefinition {
  title: string;
  description: string;
  paymentMethod: string;
  category: string;
  /** When the task targets a connector flow gap rather than a payment-method gap. */
  flow?: string;
  targetConnectors: string[];
  priority: "critical" | "high" | "medium" | "low";
  acceptanceCriteria: string[];
  runner: "opencode" | "claude-code";
  runnerModel?: string;
}

export interface SessionWithTaskInput {
  name: string;
  description?: string;
  sourcePath: string;
  strategy: SessionCopyStrategy;
  initialTask?: TaskDefinition;
}

interface UnifiedCreateSessionModalProps {
  defaultSourcePath: string;
  defaultTaskValues?: {
    title?: string;
    paymentMethod?: string;
    category?: string;
    flow?: string;
    targetConnectors?: string[];
    description?: string;
  };
  onCreate: (input: SessionWithTaskInput) => void;
  onCreateAndStart: (input: SessionWithTaskInput) => Promise<void>;
  onClose: () => void;
  wsConnected: boolean;
}

export function UnifiedCreateSessionModal({
  defaultSourcePath,
  defaultTaskValues,
  onCreate,
  onCreateAndStart,
  onClose,
  wsConnected,
}: UnifiedCreateSessionModalProps) {
  const [isCreating, setIsCreating] = useState(false);
  const [showTaskSection, setShowTaskSection] = useState(!!defaultTaskValues);
  
  // Session fields
  const [sessionName, setSessionName] = useState(defaultTaskValues?.title || "");
  const [sessionDescription, setSessionDescription] = useState("");
  const [sourcePath, setSourcePath] = useState(defaultSourcePath);
  const [strategy, setStrategy] = useState<SessionCopyStrategy>("git-worktree");
  
  // AI Runner fields
  const [runner, setRunner] = useState<"opencode" | "claude-code">("opencode");
  const [runnerModel, setRunnerModel] = useState("");
  
  // Task fields
  const [paymentMethod, setPaymentMethod] = useState(defaultTaskValues?.paymentMethod || "");
  const [category, setCategory] = useState(defaultTaskValues?.category || "");
  const [flow] = useState(defaultTaskValues?.flow || "");
  const [targetConnectors, setTargetConnectors] = useState(
    defaultTaskValues?.targetConnectors?.join(", ") || ""
  );
  const [taskDescription, setTaskDescription] = useState(defaultTaskValues?.description || "");
  const [priority, setPriority] = useState<TaskDefinition["priority"]>("medium");
  const [acceptanceCriteria, setAcceptanceCriteria] = useState<string[]>(
    flow
      ? [
          `${flow} flow returns a valid response`,
          "Error mapping follows existing connector patterns",
          "Integration test covers the new flow",
        ]
      : [
          "Authorization flow works correctly",
          "Error handling follows existing patterns",
          "Tests pass for the new payment method",
        ]
  );
  const [newCriterion, setNewCriterion] = useState("");

  const hasTask = showTaskSection && Boolean(paymentMethod || flow);
  
  const canSubmit = sessionName.trim() && sourcePath.trim() && wsConnected && !isCreating;

  const buildSessionInput = (): SessionWithTaskInput => {
    const input: SessionWithTaskInput = {
      name: sessionName.trim(),
      description: sessionDescription.trim() || undefined,
      sourcePath: sourcePath.trim(),
      strategy,
    };

    if (hasTask) {
      input.initialTask = {
        title: sessionName.trim(),
        description: taskDescription.trim(),
        paymentMethod: paymentMethod.trim(),
        category: category.trim() || "Unknown",
        flow: flow.trim() || undefined,
        targetConnectors: targetConnectors
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean),
        priority,
        acceptanceCriteria: acceptanceCriteria.filter(Boolean),
        runner,
        runnerModel: runnerModel.trim() || undefined,
      };

      // DEBUG: Log what's being sent
      console.log("[DASHBOARD] Building initialTask:", {
        runner,
        runnerModel: runnerModel.trim() || undefined,
        hasTask,
        paymentMethod: paymentMethod.trim(),
        flow: flow.trim() || undefined,
        fullTask: input.initialTask,
      });
    }

    return input;
  };

  const handleCreate = () => {
    if (!canSubmit) return;
    onCreate(buildSessionInput());
  };

  const handleCreateAndStart = async () => {
    if (!canSubmit) return;
    setIsCreating(true);
    try {
      await onCreateAndStart(buildSessionInput());
    } finally {
      setIsCreating(false);
    }
  };

  const toggleCriterion = (index: number) => {
    setAcceptanceCriteria((prev) =>
      prev.map((c, i) => (i === index ? (c ? "" : getDefaultCriteria()[i]) : c))
    );
  };

  const getDefaultCriteria = () => [
    "Authorization flow works correctly",
    "Error handling follows existing patterns",
    "Tests pass for the new payment method",
  ];

  const addCriterion = () => {
    if (newCriterion.trim()) {
      setAcceptanceCriteria((prev) => [...prev, newCriterion.trim()]);
      setNewCriterion("");
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
          width: 720,
          maxHeight: "90vh",
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
              Create Session
            </h2>
            <p style={{ margin: "4px 0 0", fontSize: 12, color: T.textMuted }}>
              Configure session and optionally define an initial task
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
          >
            ×
          </button>
        </div>

        {/* Content */}
        <div
          style={{
            flex: 1,
            overflowY: "auto",
            padding: "24px",
          }}
        >
          {!wsConnected && (
            <div
              style={{
                background: T.warnSoft,
                color: T.warn,
                padding: "12px 16px",
                borderRadius: 6,
                fontSize: 13,
                marginBottom: 20,
              }}
            >
              WebSocket not connected. Please ensure the supervisor is running.
            </div>
          )}

          {/* Session Configuration */}
          <Section title="Session Configuration">
            <Field label="Session Name *">
              <input
                value={sessionName}
                onChange={(e) => setSessionName(e.target.value)}
                placeholder="e.g., Implement Apple Pay for Stripe"
                style={inputStyle}
              />
            </Field>

            <Field label="Description">
              <textarea
                value={sessionDescription}
                onChange={(e) => setSessionDescription(e.target.value)}
                placeholder="Optional description for this session..."
                rows={2}
                style={{ ...inputStyle, resize: "vertical", fontFamily: "inherit" }}
              />
            </Field>

            <Field label="Source Folder *">
              <input
                value={sourcePath}
                onChange={(e) => setSourcePath(e.target.value)}
                placeholder="/path/to/project"
                style={inputStyle}
              />
            </Field>

            <Field label="Copy Strategy">
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {(
                  [
                    ["git-worktree", "Git worktree (fast, recommended)"],
                    ["full", "Full copy (slow, complete isolation)"],
                    ["shallow", "Shallow git clone (fastest, depth=1)"],
                  ] as [SessionCopyStrategy, string][]
                ).map(([value, label]) => (
                  <label
                    key={value}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 10,
                      cursor: "pointer",
                      fontSize: 14,
                      padding: "8px 0",
                    }}
                  >
                    <input
                      type="radio"
                      name="strategy"
                      checked={strategy === value}
                      onChange={() => setStrategy(value)}
                    />
                    <span>{label}</span>
                  </label>
                ))}
              </div>
            </Field>
          </Section>

          {/* AI Runner Selection */}
          <Section title="🤖 AI Runner">
            <div style={{ display: "flex", gap: 16, marginBottom: 16 }}>
              <RunnerCard
                name="OpenCode"
                description="Local LLM gateway"
                icon="🤖"
                isSelected={runner === "opencode"}
                onClick={() => setRunner("opencode")}
              />
              <RunnerCard
                name="Claude Code"
                description="Anthropic's CLI"
                icon="🧠"
                isSelected={runner === "claude-code"}
                onClick={() => setRunner("claude-code")}
              />
            </div>

            {runner === "claude-code" && (
              <Field label="Model (optional)">
                <input
                  value={runnerModel}
                  onChange={(e) => setRunnerModel(e.target.value)}
                  placeholder="claude-sonnet-4-6"
                  style={inputStyle}
                />
              </Field>
            )}
          </Section>

          {/* Task Definition Toggle */}
          <div
            style={{
              margin: "24px 0",
              padding: "16px 20px",
              background: hasTask ? T.accentSoft : T.bg,
              border: `2px solid ${hasTask ? T.accent : T.border}`,
              borderRadius: 8,
              cursor: "pointer",
              transition: "all 150ms",
            }}
            onClick={() => setShowTaskSection(!showTaskSection)}
          >
            <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
              <input
                type="checkbox"
                checked={showTaskSection}
                onChange={() => {}}
                style={{ pointerEvents: "none" }}
              />
              <div>
                <div style={{ fontWeight: 600, fontSize: 15 }}>
                  Include Initial Task
                </div>
                <div style={{ fontSize: 12, color: T.textMuted, marginTop: 2 }}>
                  Define a task that will auto-start when the session begins
                </div>
              </div>
            </div>
          </div>

          {/* Task Definition Section */}
          {showTaskSection && (
            <Section title="Task Definition">
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "1fr 1fr",
                  gap: 16,
                }}
              >
                <Field label="Payment Method">
                  <input
                    value={paymentMethod}
                    onChange={(e) => setPaymentMethod(e.target.value)}
                    placeholder="e.g., Apple Pay"
                    style={inputStyle}
                  />
                </Field>

                <Field label="Category">
                  <input
                    value={category}
                    onChange={(e) => setCategory(e.target.value)}
                    placeholder="e.g., WALLET"
                    style={inputStyle}
                  />
                </Field>
              </div>

              <Field label="Target Connectors">
                <input
                  value={targetConnectors}
                  onChange={(e) => setTargetConnectors(e.target.value)}
                  placeholder="Comma-separated: Stripe, Adyen"
                  style={inputStyle}
                />
              </Field>

              <Field label="Priority">
                <select
                  value={priority}
                  onChange={(e) => setPriority(e.target.value as TaskDefinition["priority"])}
                  style={inputStyle}
                >
                  <option value="critical">Critical</option>
                  <option value="high">High</option>
                  <option value="medium">Medium</option>
                  <option value="low">Low</option>
                </select>
              </Field>

              <Field label="Acceptance Criteria">
                <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                  {acceptanceCriteria.map((criterion, index) => (
                    <label
                      key={index}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 10,
                        cursor: "pointer",
                        fontSize: 14,
                      }}
                    >
                      <input
                        type="checkbox"
                        checked={!!criterion}
                        onChange={() => toggleCriterion(index)}
                      />
                      <span style={{ color: criterion ? T.text : T.textSubtle }}>
                        {criterion || getDefaultCriteria()[index]}
                      </span>
                    </label>
                  ))}
                </div>
                <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
                  <input
                    value={newCriterion}
                    onChange={(e) => setNewCriterion(e.target.value)}
                    placeholder="Add custom criterion..."
                    style={{ ...inputStyle, flex: 1 }}
                    onKeyDown={(e) => e.key === "Enter" && addCriterion()}
                  />
                  <button
                    onClick={addCriterion}
                    style={{
                      padding: "8px 16px",
                      borderRadius: 6,
                      border: `1px solid ${T.border}`,
                      background: T.bg,
                      cursor: "pointer",
                    }}
                  >
                    Add
                  </button>
                </div>
              </Field>

              <Field label="Task Description">
                <textarea
                  value={taskDescription}
                  onChange={(e) => setTaskDescription(e.target.value)}
                  placeholder="Describe the implementation requirements..."
                  rows={4}
                  style={{ ...inputStyle, resize: "vertical", fontFamily: "inherit" }}
                />
              </Field>
            </Section>
          )}
        </div>

        {/* Footer */}
        <div
          style={{
            padding: "20px 24px",
            borderTop: `1px solid ${T.border}`,
            display: "flex",
            justifyContent: "flex-end",
            gap: 12,
            background: T.bgSidebar,
          }}
        >
          <button
            onClick={onClose}
            disabled={isCreating}
            style={{
              padding: "10px 20px",
              borderRadius: 6,
              border: `1px solid ${T.border}`,
              background: "transparent",
              color: T.text,
              fontSize: 14,
              cursor: isCreating ? "not-allowed" : "pointer",
              opacity: isCreating ? 0.6 : 1,
            }}
          >
            Cancel
          </button>
          <button
            onClick={handleCreate}
            disabled={!canSubmit}
            style={{
              padding: "10px 20px",
              borderRadius: 6,
              border: `1px solid ${T.border}`,
              background: canSubmit ? T.bgElev : T.border,
              color: canSubmit ? T.text : T.textSubtle,
              fontSize: 14,
              fontWeight: 600,
              cursor: canSubmit ? "pointer" : "not-allowed",
            }}
          >
            Create Session
          </button>
          <button
            onClick={handleCreateAndStart}
            disabled={!canSubmit}
            style={{
              padding: "10px 24px",
              borderRadius: 6,
              border: "none",
              background: canSubmit ? T.accent : T.border,
              color: "#fff",
              fontSize: 14,
              fontWeight: 600,
              cursor: canSubmit ? "pointer" : "not-allowed",
              display: "flex",
              alignItems: "center",
              gap: 8,
            }}
          >
            {isCreating ? (
              <>
                <span style={{ animation: "spin 1s linear infinite" }}>⟳</span>
                Creating...
              </>
            ) : (
              <>
                Create & Start →
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: 24 }}>
      <h3
        style={{
          fontSize: 13,
          fontWeight: 700,
          color: T.textMuted,
          textTransform: "uppercase",
          letterSpacing: 1,
          margin: "0 0 16px 0",
        }}
      >
        {title}
      </h3>
      {children}
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: 16 }}>
      <label
        style={{
          display: "block",
          fontSize: 13,
          fontWeight: 600,
          color: T.textMuted,
          marginBottom: 6,
        }}
      >
        {label}
      </label>
      {children}
    </div>
  );
}

function RunnerCard({
  name,
  description,
  icon,
  isSelected,
  onClick,
}: {
  name: string;
  description: string;
  icon: string;
  isSelected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        flex: 1,
        padding: "16px 20px",
        borderRadius: 8,
        border: `2px solid ${isSelected ? T.accent : T.border}`,
        background: isSelected ? T.accentSoft : T.bg,
        cursor: "pointer",
        textAlign: "left",
        transition: "all 150ms",
      }}
    >
      <div style={{ fontSize: 24, marginBottom: 8 }}>{icon}</div>
      <div
        style={{
          fontWeight: 600,
          fontSize: 15,
          color: isSelected ? T.accent : T.text,
          marginBottom: 4,
        }}
      >
        {name}
        {isSelected && <span style={{ marginLeft: 8 }}>●</span>}
      </div>
      <div style={{ fontSize: 12, color: T.textMuted }}>{description}</div>
    </button>
  );
}

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "10px 14px",
  borderRadius: 6,
  border: `1px solid ${T.border}`,
  background: T.bg,
  color: T.text,
  fontSize: 14,
  outline: "none",
  fontFamily: "inherit",
  boxSizing: "border-box",
};
