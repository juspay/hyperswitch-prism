import type { CheckpointState } from "../hooks/usePipeline";
import { PIPELINE } from "../hooks/usePipeline";
import { T } from "../theme";

// Grace 2.3_codegen.md workflow: task → preflight → L2_planning → L3_analysis → implementation → compiler → grpc_test
const PHASES: Array<{ label: string; ids: string[] }> = [
  { label: "Intake", ids: ["task", "preflight"] },
  { label: "Planning", ids: ["l2_planning", "l2_review", "l3_analysis", "l3_review"] },
  { label: "Implementation", ids: ["implementation"] },
  {
    label: "Verification",
    ids: ["compiler", "grpc_test", "pr_review", "regression"],
  },
];

const GUTTER_WIDTH = 36;
const INDICATOR_SIZE = 18;

function Indicator({
  state,
  pulsing,
}: {
  state: CheckpointState;
  pulsing: boolean;
}) {
  const base: React.CSSProperties = {
    width: INDICATOR_SIZE,
    height: INDICATOR_SIZE,
    borderRadius: "50%",
    flexShrink: 0,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    fontSize: 10,
    fontWeight: 700,
    color: "#fff",
    position: "relative",
    zIndex: 2,
    boxShadow: "0 0 0 3px " + T.bgSidebar,
  };
  const pulseRing: React.CSSProperties | undefined = pulsing
    ? {
        position: "absolute",
        inset: -4,
        borderRadius: "50%",
        boxShadow: `0 0 0 3px ${T.accentSoft}`,
        animation: "pulse 1.8s ease-in-out infinite",
      }
    : undefined;

  if (state.status === "running") {
    return (
      <div style={{ position: "relative", zIndex: 2 }}>
        {pulseRing && <div style={pulseRing} />}
        <div
          style={{
            ...base,
            background: T.bgElev,
            border: `2px solid ${T.accent}`,
            borderTopColor: "transparent",
            animation: "spin 0.9s linear infinite",
          }}
        />
      </div>
    );
  }
  if (state.status === "passed") {
    return <div style={{ ...base, background: T.success }}>✓</div>;
  }
  if (state.status === "failed") {
    return <div style={{ ...base, background: T.error }}>✕</div>;
  }
  if (state.status === "skipped") {
    return <div style={{ ...base, background: T.warn }}>−</div>;
  }
  return (
    <div
      style={{
        ...base,
        background: T.bgElev,
        border: `2px solid ${T.borderStrong}`,
      }}
    >
      {pulsing && <div style={pulseRing!} />}
    </div>
  );
}

interface Row {
  id: string;
  name: string;
  type: "auto" | "human";
  globalIdx: number;
  isFirst: boolean;
  isLast: boolean;
}

function StepRow({
  row,
  state,
  isSelected,
  onSelect,
}: {
  row: Row;
  state: CheckpointState;
  isSelected: boolean;
  onSelect: () => void;
}) {
  const isWaiting = !!state.waiting;
  const isRunning = state.status === "running";
  const isPassed = state.status === "passed";
  const isFailed = state.status === "failed";

  // Color the connecting rail segment based on whether the step *above* has completed
  const topRailColor = row.isFirst
    ? "transparent"
    : isPassed || isRunning || isFailed
      ? T.accent
      : T.border;
  const bottomRailColor = row.isLast
    ? "transparent"
    : isPassed
      ? T.accent
      : T.border;

  return (
    <button
      onClick={onSelect}
      style={{
        display: "flex",
        alignItems: "stretch",
        width: "100%",
        padding: 0,
        margin: 0,
        background: "transparent",
        border: "none",
        cursor: "pointer",
        textAlign: "left",
        color: T.text,
        position: "relative",
      }}
    >
      {/* Gutter with connecting rail + indicator */}
      <div
        style={{
          width: GUTTER_WIDTH,
          flexShrink: 0,
          position: "relative",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        {/* Top half of the rail */}
        <div
          style={{
            position: "absolute",
            top: 0,
            bottom: "50%",
            left: "50%",
            width: 2,
            marginLeft: -1,
            background: topRailColor,
          }}
        />
        {/* Bottom half of the rail */}
        <div
          style={{
            position: "absolute",
            top: "50%",
            bottom: 0,
            left: "50%",
            width: 2,
            marginLeft: -1,
            background: bottomRailColor,
          }}
        />
        <Indicator state={state} pulsing={isWaiting && !isRunning} />
      </div>

      {/* Content card */}
      <div
        style={{
          flex: 1,
          minWidth: 0,
          margin: "3px 12px 3px 4px",
          padding: "9px 12px",
          borderRadius: 8,
          background: isSelected
            ? T.accentSoft
            : isRunning
              ? "rgba(160, 82, 45, 0.06)"
              : "transparent",
          border: isSelected
            ? `1px solid ${T.accent}`
            : isRunning
              ? `1px solid ${T.accentSoft}`
              : "1px solid transparent",
          transition: "background 140ms, border-color 140ms",
        }}
        onMouseEnter={(e) => {
          if (!isSelected && !isRunning) {
            e.currentTarget.style.background = "#f5ead0";
          }
        }}
        onMouseLeave={(e) => {
          if (!isSelected && !isRunning) {
            e.currentTarget.style.background = "transparent";
          }
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            marginBottom: 3,
          }}
        >
          <span
            style={{
              fontSize: 9,
              color: T.textSubtle,
              fontFamily: "ui-monospace, monospace",
              fontWeight: 600,
              letterSpacing: 0.4,
            }}
          >
            {String(row.globalIdx + 1).padStart(2, "0")}
          </span>
          <div
            style={{
              fontSize: 13,
              fontWeight: isSelected || isRunning ? 600 : 500,
              color: T.text,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              lineHeight: 1.3,
              flex: 1,
              minWidth: 0,
            }}
          >
            {row.name}
          </div>
        </div>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            fontSize: 10,
            color: T.textMuted,
            marginLeft: 18,
          }}
        >
          <span
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              padding: "1px 6px",
              borderRadius: 999,
              background: row.type === "human" ? T.warnSoft : T.codeBg,
              color: row.type === "human" ? T.warn : T.textMuted,
              fontWeight: 500,
              fontSize: 9,
              textTransform: "uppercase",
              letterSpacing: 0.4,
            }}
          >
            {row.type === "human" ? "human" : "auto"}
          </span>
          {state.retries > 0 && (
            <span
              style={{
                padding: "1px 6px",
                borderRadius: 999,
                background: T.warnSoft,
                color: T.warn,
                fontWeight: 600,
                fontSize: 9,
              }}
            >
              retry {state.retries}
            </span>
          )}
          {isWaiting && (
            <span
              style={{
                padding: "1px 6px",
                borderRadius: 999,
                background: T.accentSoft,
                color: T.accent,
                fontWeight: 600,
                fontSize: 9,
                textTransform: "uppercase",
                letterSpacing: 0.4,
              }}
            >
              awaiting
            </span>
          )}
        </div>
      </div>
    </button>
  );
}

export function Sidebar({
  states,
  selectedId,
  onSelect,
  runId,
  wsStatus,
}: {
  states: Record<string, CheckpointState>;
  selectedId: string;
  onSelect: (id: string) => void;
  runId?: string;
  wsStatus: "connecting" | "open" | "closed";
}) {
  return (
    <aside
      style={{
        width: 328,
        background: T.bgSidebar,
        borderRight: `1px solid ${T.border}`,
        display: "flex",
        flexDirection: "column",
        height: "100vh",
      }}
    >
      <Header runId={runId} wsStatus={wsStatus} />

      <div style={{ flex: 1, overflowY: "auto", padding: "14px 0 24px" }}>
        {PHASES.map((phase) => (
          <div key={phase.label} style={{ marginBottom: 4 }}>
            <div
              style={{
                padding: "10px 24px 6px",
                fontSize: 10,
                fontWeight: 700,
                textTransform: "uppercase",
                letterSpacing: 1.2,
                color: T.textSubtle,
                display: "flex",
                alignItems: "center",
                gap: 8,
              }}
            >
              <span>{phase.label}</span>
              <span
                style={{
                  flex: 1,
                  height: 1,
                  background: T.border,
                }}
              />
            </div>
            <div style={{ padding: "0 12px" }}>
              {phase.ids.map((id, i) => {
                const cp = PIPELINE.find((p) => p.id === id);
                if (!cp) return null; // Skip if checkpoint not in pipeline
                const globalIdx = PIPELINE.findIndex((p) => p.id === id);
                const state = states[id] ?? {
                  id,
                  status: "idle" as const,
                  retries: 0,
                };
                return (
                  <StepRow
                    key={id}
                    row={{
                      id,
                      name: cp.name,
                      type: cp.type,
                      globalIdx,
                      isFirst: i === 0,
                      isLast: i === phase.ids.length - 1,
                    }}
                    state={state}
                    isSelected={selectedId === id}
                    onSelect={() => onSelect(id)}
                  />
                );
              })}
            </div>
          </div>
        ))}
      </div>

      <Footer states={states} />
    </aside>
  );
}

function Header({
  runId,
  wsStatus,
}: {
  runId?: string;
  wsStatus: "connecting" | "open" | "closed";
}) {
  const dotColor =
    wsStatus === "open"
      ? T.success
      : wsStatus === "connecting"
        ? T.warn
        : T.error;
  const dotHalo =
    wsStatus === "open"
      ? T.successSoft
      : wsStatus === "connecting"
        ? T.warnSoft
        : T.errorSoft;
  return (
    <div
      style={{
        padding: "22px 24px 18px",
        borderBottom: `1px solid ${T.border}`,
        background: `linear-gradient(180deg, ${T.bgElev} 0%, ${T.bgSidebar} 100%)`,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <div
          style={{
            width: 34,
            height: 34,
            borderRadius: 10,
            background: `linear-gradient(135deg, ${T.accent}, #c97a45)`,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: "#fff",
            fontWeight: 700,
            fontSize: 15,
            boxShadow: "0 2px 6px rgba(160, 82, 45, 0.25)",
          }}
        >
          C
        </div>
        <div style={{ minWidth: 0 }}>
          <div
            style={{
              fontSize: 15,
              fontWeight: 700,
              color: T.text,
              lineHeight: 1.2,
            }}
          >
            10XGRACE
          </div>
          <div style={{ fontSize: 10, color: T.textMuted, marginTop: 2 }}>
            spec-driven dev · 12 checkpoints
          </div>
        </div>
      </div>
      <div
        style={{
          marginTop: 16,
          display: "flex",
          alignItems: "center",
          gap: 8,
          fontSize: 11,
          padding: "8px 10px",
          background: T.bgElev,
          border: `1px solid ${T.border}`,
          borderRadius: 8,
        }}
      >
        <span
          style={{
            width: 7,
            height: 7,
            borderRadius: "50%",
            background: dotColor,
            boxShadow: `0 0 0 3px ${dotHalo}`,
            flexShrink: 0,
          }}
        />
        <span style={{ color: T.text, fontWeight: 500 }}>
          {wsStatus === "open"
            ? "Engine connected"
            : wsStatus === "connecting"
              ? "Connecting…"
              : "Engine offline"}
        </span>
        <span
          style={{
            marginLeft: "auto",
            fontSize: 10,
            color: T.textSubtle,
            fontFamily: "ui-monospace, monospace",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            maxWidth: 110,
          }}
          title={runId}
        >
          {runId ? runId.slice(4, 18) : "no run"}
        </span>
      </div>
    </div>
  );
}

function Footer({ states }: { states: Record<string, CheckpointState> }) {
  const total = PIPELINE.length;
  const passed = PIPELINE.filter((p) => states[p.id]?.status === "passed").length;
  const failed = PIPELINE.filter((p) => states[p.id]?.status === "failed").length;
  const pct = Math.round((passed / total) * 100);
  return (
    <div
      style={{
        padding: "14px 24px 18px",
        borderTop: `1px solid ${T.border}`,
        background: T.bgElev,
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "baseline",
          fontSize: 11,
          marginBottom: 8,
        }}
      >
        <span style={{ color: T.text, fontWeight: 600, letterSpacing: 0.3 }}>
          Progress
        </span>
        <span style={{ color: T.textMuted }}>
          <span style={{ color: T.success, fontWeight: 600 }}>{passed}</span>
          {failed > 0 && (
            <>
              {" · "}
              <span style={{ color: T.error, fontWeight: 600 }}>{failed} failed</span>
            </>
          )}
          {" / "}
          {total}
        </span>
      </div>
      <div
        style={{
          height: 6,
          background: T.border,
          borderRadius: 999,
          overflow: "hidden",
          position: "relative",
        }}
      >
        <div
          style={{
            width: `${pct}%`,
            height: "100%",
            background: `linear-gradient(90deg, ${T.accent}, ${T.success})`,
            transition: "width 300ms",
            borderRadius: 999,
          }}
        />
      </div>
    </div>
  );
}
