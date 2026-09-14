import type { CheckpointState } from "../hooks/usePipeline";
import { PIPELINE } from "../hooks/usePipeline";
import { T } from "../theme";

// Superset across all workflows. The actually-rendered subset comes from
// `effectiveActiveIds` inside the Sidebar function — either the engine's
// `pipeline:list` event (post-submit) or one of these synthetic preview
// lists derived from the in-form Task-kind picker (pre-submit). Rows not
// in the active list are HIDDEN, not greyed — keeps the sidebar tight to
// just the work this session will actually do.

const STANDARD_PIPELINE_IDS: ReadonlyArray<string> = [
  "task",
  "preflight",
  "l2_planning",
  "l2_review",
  "l3_analysis",
  "l3_review",
  "implementation",
  "compiler",
  "grpc_test",
  "pr_review",
  "regression",
];

const NEW_CONNECTOR_PIPELINE_IDS: ReadonlyArray<string> = [
  "task",
  "preflight",
  "scaffold",
  // Core flows — always run. Pre-auth flows are added at engine run-time
  // based on the techspec; they show up via the real `pipeline:list` event
  // after submit, so the pre-submit preview just shows the 6 core ones.
  "impl_authorize",
  "impl_psync",
  "impl_capture",
  "impl_refund",
  "impl_rsync",
  "impl_void",
  "compiler",
  "grpc_test",
  "pr_review",
  "regression",
];
const PHASES: Array<{ label: string; ids: string[] }> = [
  { label: "Intake", ids: ["task", "preflight", "scaffold"] },
  { label: "Planning", ids: ["l2_planning", "l2_review", "l3_analysis", "l3_review"] },
  {
    label: "Implementation",
    ids: [
      "implementation",
      // Per-flow subagents (`.gracerules` PHASE 3 order: pre-auth then core).
      "impl_create_access_token",
      "impl_create_order",
      "impl_create_connector_customer",
      "impl_payment_method_token",
      "impl_create_session_token",
      "impl_authorize",
      "impl_psync",
      "impl_capture",
      "impl_refund",
      "impl_rsync",
      "impl_void",
    ],
  },
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
  /** 1-based sequential position within the *visible* pipeline (post-reshape). */
  displayIdx: number;
  isFirst: boolean;
  isLast: boolean;
  /** Optional hover tooltip explaining flow dependencies, e.g.
   *  "Runs after: Create Order". Only set for impl_* checkpoints whose
   *  flow has declared+implicit deps present in this pipeline run. */
  depsTooltip?: string;
}

// Mirror of packages/core/src/checkpoints/flow-checkpoint.ts FLOWS registry
// — kept in sync manually with the canonical core copy. The dashboard
// doesn't depend on @10xgrace/core (no runtime import), so this static map
// is duplicated here just for the StepRow tooltip. Update both sides when
// adding a new flow.
const FLOW_DEPS_MIRROR: Record<
  string,
  { kind: "pre-auth" | "core"; declaredDeps: string[] }
> = {
  impl_create_access_token:       { kind: "pre-auth", declaredDeps: [] },
  impl_create_order:              { kind: "pre-auth", declaredDeps: [] },
  impl_create_connector_customer: { kind: "pre-auth", declaredDeps: [] },
  impl_payment_method_token:      { kind: "pre-auth", declaredDeps: [] },
  impl_create_session_token:      { kind: "pre-auth", declaredDeps: [] },
  impl_authorize:                 { kind: "core",     declaredDeps: [] },
  impl_psync:                     { kind: "core",     declaredDeps: ["impl_authorize"] },
  impl_capture:                   { kind: "core",     declaredDeps: ["impl_authorize"] },
  impl_refund:                    { kind: "core",     declaredDeps: ["impl_capture"] },
  impl_rsync:                     { kind: "core",     declaredDeps: ["impl_refund"] },
  impl_void:                      { kind: "core",     declaredDeps: ["impl_authorize"] },
};

function computeFlowDepsTooltip(
  stepId: string,
  visibleIds: ReadonlyArray<string>,
): string | undefined {
  const flow = FLOW_DEPS_MIRROR[stepId];
  if (!flow) return undefined;
  const visible = new Set(visibleIds);
  const deps = new Set<string>();
  for (const d of flow.declaredDeps) if (visible.has(d)) deps.add(d);
  if (flow.kind === "core") {
    for (const [id, f] of Object.entries(FLOW_DEPS_MIRROR)) {
      if (f.kind === "pre-auth" && visible.has(id)) deps.add(id);
    }
  }
  if (deps.size === 0) return undefined;
  const pretty = (id: string) =>
    id
      .replace(/^impl_/, "")
      .replace(/_/g, " ")
      .replace(/\b\w/g, (c) => c.toUpperCase());
  return `Runs after: ${[...deps].map(pretty).join(", ")}`;
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
      title={row.depsTooltip}
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
            {String(row.displayIdx).padStart(2, "0")}
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
  activePipelineIds,
  previewTaskKind,
}: {
  states: Record<string, CheckpointState>;
  selectedId: string;
  onSelect: (id: string) => void;
  runId?: string;
  wsStatus: "connecting" | "open" | "closed";
  /**
   * Authoritative list of checkpoint ids this run will execute, sent by
   * the engine on `pipeline:list` at run start. When set, rows not in it
   * are hidden. When null (no run yet), we fall back to the
   * `previewTaskKind` synthetic list, then to "show all" as a final
   * fallback.
   */
  activePipelineIds?: string[] | null;
  /**
   * Pre-submit hint from the Task Definition form's in-form Task-kind
   * picker (lifted via WorkflowPage). When the engine hasn't yet emitted
   * `pipeline:list`, this drives the sidebar — rows for the OTHER
   * workflow are hidden so the user sees only the work this task type
   * actually does.
   */
  previewTaskKind?: "standard" | "integrate" | null;
}) {
  // Resolve which checkpoints to render. Priority:
  //   1. `activePipelineIds` — engine truth via `pipeline:list`. ALWAYS
  //      wins when set. Includes post-rebuild updates (engine re-emits
  //      pipeline:list after `task` passes if workflowType demanded a
  //      different sequence). Previously gated on `taskStepPassed`, but
  //      that race-condition'd when the dashboard joined late and missed
  //      the task pass event — `previewTaskKind="standard"` would then
  //      override and hide the actually-running scaffold + impl_* rows.
  //   2. `previewTaskKind` — pre-submit hint from the in-form picker.
  //      Only used when the engine hasn't told us anything yet.
  //   3. null fallback — show every row in the PIPELINE superset.
  const effectiveActiveIds: ReadonlySet<string> | null = (() => {
    if (activePipelineIds) return new Set(activePipelineIds);
    if (previewTaskKind === "standard") return new Set(STANDARD_PIPELINE_IDS);
    if (previewTaskKind === "integrate")
      return new Set(NEW_CONNECTOR_PIPELINE_IDS);
    return null;
  })();

  // Sequential 1-based display index across all visible phases. Avoids
  // exposing PIPELINE's master indexes (which jumped 01, 02, 04 … 20, 22, 23
  // when reshape filtered the visible set). Built once per render so each
  // row's badge shows its position in the *current* run, not in the canonical
  // 24-step superset.
  const displayIndexById = new Map<string, number>();
  {
    let n = 0;
    for (const phase of PHASES) {
      for (const id of phase.ids) {
        if (effectiveActiveIds && !effectiveActiveIds.has(id)) continue;
        n += 1;
        displayIndexById.set(id, n);
      }
    }
  }
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
        {PHASES.map((phase) => {
          // Hide rows not in the effective active set (engine-confirmed
          // or pre-submit-preview). If the whole phase collapses to
          // nothing (e.g. Planning for new-connector), drop the section
          // header too so the sidebar stays tight.
          const visibleIds = effectiveActiveIds
            ? phase.ids.filter((id) => effectiveActiveIds.has(id))
            : phase.ids;
          if (visibleIds.length === 0) return null;
          return (
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
              {visibleIds.map((id, i) => {
                const cp = PIPELINE.find((p) => p.id === id);
                if (!cp) return null;
                const displayIdx = displayIndexById.get(id) ?? i + 1;
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
                      displayIdx,
                      isFirst: i === 0,
                      isLast: i === visibleIds.length - 1,
                      depsTooltip: computeFlowDepsTooltip(id, visibleIds),
                    }}
                    state={state}
                    isSelected={selectedId === id}
                    onSelect={() => onSelect(id)}
                  />
                );
              })}
            </div>
          </div>
          );
        })}
      </div>

      <Footer
        states={states}
        visibleIds={Array.from(displayIndexById.keys())}
      />
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
            spec-driven dev
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

function Footer({
  states,
  visibleIds,
}: {
  states: Record<string, CheckpointState>;
  /** Checkpoints actually in this run's pipeline (post-reshape). Used as the
   * progress denominator so the bar reads X/N where N matches what the
   * sidebar shows, not the canonical 24-step superset. */
  visibleIds: readonly string[];
}) {
  const denominator = visibleIds.length > 0 ? visibleIds : PIPELINE.map((p) => p.id);
  const total = denominator.length;
  const passed = denominator.filter((id) => states[id]?.status === "passed").length;
  const failed = denominator.filter((id) => states[id]?.status === "failed").length;
  const pct = total > 0 ? Math.round((passed / total) * 100) : 0;
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
