import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { SidebarLayout } from "../components/NavigationSidebar";
import { DiffViewer } from "../components/DiffViewer";
import {
  usePrResolver,
  type BoardCard,
  type BoardStep,
  type GrpcTestResultRecord,
  type GrpcTestStepResult,
  type PrMachine,
  type PrMachineStatus,
} from "../hooks/usePrResolver";
import { T } from "../theme";

const CONTROL_WS_PORT =
  (import.meta.env.VITE_WS_PORT as string | undefined) ?? "3142";
const CONTROL_WS_URL = `ws://${location.hostname}:${CONTROL_WS_PORT}`;

type StageStatus = "idle" | "running" | "passed" | "failed";

interface StageDef {
  id: string;
  phase: string;
  label: string;
}

const STAGES: StageDef[] = [
  { phase: "Discovery", id: "notice", label: "Noticed" },
  { phase: "Discovery", id: "react", label: "Reacted 👀" },
  { phase: "Preparation", id: "pr_open", label: "PR still open" },
  { phase: "Preparation", id: "checkout", label: "Checkout branch" },
  { phase: "Preparation", id: "baseline", label: "Baseline build" },
  { phase: "Preparation", id: "threads", label: "Threads unresolved" },
  { phase: "Resolution", id: "resolve", label: "Resolve comments" },
  { phase: "Verification", id: "build", label: "Cargo build" },
  { phase: "Verification", id: "clippy", label: "Cargo clippy" },
  { phase: "Verification", id: "grpc_test", label: "gRPC test" },
  { phase: "Verification", id: "fmt", label: "Cargo fmt" },
  { phase: "Verification", id: "scope", label: "Scope check" },
  { phase: "Review", id: "approval", label: "Approval" },
  { phase: "Finalize", id: "commit", label: "Commit" },
  { phase: "Finalize", id: "push", label: "Push" },
  { phase: "Finalize", id: "reply", label: "Reply" },
];

const MACHINE_PHASE_ORDER: Record<PrMachineStatus, number> = {
  noticed: 0,
  preparing: 1,
  resolving: 2,
  verifying: 3,
  awaiting_approval: 4,
  committing: 5,
  pushed: 6,
  rejected: 6,
  failed: 6,
};

/**
 * Per-PR detail view. Mirrors Byne's WorkflowPage layout: phases + checkpoint
 * rows on the left, current stage details + diff/approval UI on the right,
 * full event timeline beneath. Live via the same WS the Kanban page uses.
 */
export function PrResolverDetailPage() {
  const { prNumber: prNumberRaw } = useParams<{ prNumber: string }>();
  const prNumber = Number(prNumberRaw);

  const {
    enabled,
    running,
    controlStatus,
    githubRepo,
    trigger,
    board,
    processedThreads,
    buildFailures,
    prMachines,
    autoApprove,
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
  } = usePrResolver(CONTROL_WS_URL);

  const card = useMemo<BoardCard | null>(() => {
    if (!Number.isFinite(prNumber)) return null;
    for (const column of [
      board.inProgress,
      board.queued,
      board.completed,
      board.failed,
    ]) {
      const match = column.find((c) => c.pr === prNumber);
      if (match) return match;
    }
    return null;
  }, [board, prNumber]);

  const machine = prMachines[String(prNumber)] ?? null;
  const stageStatuses = useMemo(
    () => computeStageStatuses(machine, card?.steps ?? []),
    [machine, card]
  );

  // Auto-select the current stage when entering or when status moves. We
  // wait until the machine has actually landed via WS — otherwise the first
  // render (machine = null, everything `idle`) would lock the rail onto
  // "Noticed" and never reselect once the real status arrives.
  const [selectedStageId, setSelectedStageId] = useState<string | null>(null);
  useEffect(() => {
    if (!machine) return;
    const running = STAGES.find((s) => stageStatuses[s.id] === "running");
    const lastDone = [...STAGES]
      .reverse()
      .find((s) => stageStatuses[s.id] === "passed" || stageStatuses[s.id] === "failed");
    const next = running ?? lastDone ?? STAGES[0]!;
    setSelectedStageId((prev) => prev ?? next.id);
  }, [stageStatuses, machine]);

  // Lazy-load the diff when the user lands on the Approval stage.
  useEffect(() => {
    if (machine?.status === "awaiting_approval") {
      requestDiff(prNumber);
    }
  }, [machine?.status, prNumber, requestDiff]);

  const selectedStage =
    STAGES.find((s) => s.id === selectedStageId) ?? STAGES[0]!;

  const historicalThreads = useMemo(() => {
    return Object.entries(processedThreads)
      .filter(([, entry]) => entry.pr_number === prNumber)
      .map(([threadId, entry]) => ({ threadId, ...entry }));
  }, [processedThreads, prNumber]);

  return (
    <SidebarLayout>
      <style>{`
        @keyframes prResolverPulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.4; }
        }
        @keyframes spin {
          to { transform: rotate(360deg); }
        }
      `}</style>
      <div
        style={{
          minHeight: "100vh",
          background: T.bg,
          color: T.text,
          fontFamily:
            "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
          display: "flex",
          flexDirection: "column",
        }}
      >
        <Header
          prNumber={prNumber}
          machine={machine}
          enabled={enabled}
          running={running}
          githubRepo={githubRepo}
          trigger={trigger}
          controlStatus={controlStatus}
          autoApprove={autoApprove}
          setAutoApprove={setAutoApprove}
        />

        {lastError && (
          <div
            style={{
              padding: "10px 32px",
              background: T.errorSoft,
              color: T.error,
              fontSize: 12,
              borderBottom: `1px solid ${T.error}33`,
            }}
          >
            {lastError.kind}: {lastError.message}
          </div>
        )}

        {!Number.isFinite(prNumber) ? (
          <NotFound message="Invalid PR number in URL." />
        ) : !machine && !card ? (
          <NotFound
            message={
              historicalThreads.length > 0
                ? "This PR's in-flight machine has been cleared, but thread history is below."
                : "No record of this PR yet — the resolver hasn't picked it up."
            }
          />
        ) : (
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "280px minmax(0, 1fr)",
              gap: 0,
              flex: 1,
              minHeight: 0,
            }}
          >
            <StageRail
              stages={STAGES}
              statuses={stageStatuses}
              selectedId={selectedStage.id}
              onSelect={setSelectedStageId}
            />
            <MainPanel
              stage={selectedStage}
              status={stageStatuses[selectedStage.id]}
              machine={machine}
              card={card}
              prNumber={prNumber}
              running={running}
              autoApprove={autoApprove}
              diff={diffForPr[String(prNumber)]?.diff ?? machine?.diffPreview ?? ""}
              buildFailure={buildFailures[String(prNumber)] ?? null}
              resolverStream={resolverStreams[String(prNumber)] ?? []}
              grpcServerLog={grpcServerLogs[String(prNumber)] ?? []}
              onApprove={(note) => approvePr(prNumber, note)}
              onReject={(reason) => rejectPr(prNumber, reason)}
              onRetry={() => retryPr(prNumber)}
              onRequestChanges={(feedback) => requestChanges(prNumber, feedback)}
              onRequestDiff={() => requestDiff(prNumber)}
            />
          </div>
        )}

        {historicalThreads.length > 0 && (
          <HistorySection threads={historicalThreads} />
        )}
      </div>
    </SidebarLayout>
  );
}

// ─── Header ──────────────────────────────────────────────────────────

function Header({
  prNumber,
  machine,
  enabled,
  running,
  githubRepo,
  trigger,
  controlStatus,
  autoApprove,
  setAutoApprove,
}: {
  prNumber: number;
  machine: PrMachine | null;
  enabled: boolean;
  running: boolean;
  githubRepo: string;
  trigger: string;
  controlStatus: "connecting" | "open" | "closed";
  autoApprove: boolean;
  setAutoApprove: (v: boolean) => void;
}) {
  const machineStatus = machine?.status ?? null;
  const statusInfo = machineStatusLabel(machineStatus);
  const prUrl =
    githubRepo && Number.isFinite(prNumber)
      ? `https://github.com/${githubRepo}/pull/${prNumber}`
      : null;

  return (
    <header
      style={{
        padding: "20px 32px",
        borderBottom: `1px solid ${T.border}`,
        background: T.bgElev,
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        flexWrap: "wrap",
        gap: 12,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
        <Link
          to="/pr-resolver"
          style={{
            color: T.textMuted,
            fontSize: 12,
            textDecoration: "none",
            padding: "6px 10px",
            borderRadius: 6,
            border: `1px solid ${T.border}`,
            background: T.bg,
          }}
        >
          ← Back
        </Link>
        <div>
          <h1
            style={{
              margin: 0,
              fontSize: 18,
              fontWeight: 700,
              display: "flex",
              alignItems: "center",
              gap: 10,
            }}
          >
            PR #{Number.isFinite(prNumber) ? prNumber : "?"}
            {prUrl && (
              <a
                href={prUrl}
                target="_blank"
                rel="noopener noreferrer"
                style={{ fontSize: 11, color: T.accent, textDecoration: "none" }}
              >
                open on GitHub ↗
              </a>
            )}
          </h1>
          <span style={{ fontSize: 12, color: T.textMuted }}>
            {githubRepo || "(no repo set)"} · trigger{" "}
            <code style={code}>{trigger || "@trigger"}</code>
          </span>
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <ConnDot status={controlStatus} />
        <StatusPill
          label={statusInfo.label}
          color={statusInfo.color}
          pulse={machineStatus === "resolving" || machineStatus === "preparing" || machineStatus === "committing" || running}
        />
        <AutoApproveToggle
          autoApprove={autoApprove}
          disabled={!enabled || controlStatus !== "open"}
          onToggle={setAutoApprove}
        />
      </div>
    </header>
  );
}

function AutoApproveToggle({
  autoApprove,
  disabled,
  onToggle,
}: {
  autoApprove: boolean;
  disabled?: boolean;
  onToggle: (v: boolean) => void;
}) {
  return (
    <button
      type="button"
      title={
        disabled
          ? "Enable the resolver first"
          : autoApprove
            ? "Auto-approve is ON for this session — pushes happen without manual confirmation"
            : "Auto-approve is OFF — every PR waits for your approval"
      }
      disabled={disabled}
      onClick={() => onToggle(!autoApprove)}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 8,
        padding: "4px 10px 4px 6px",
        borderRadius: 999,
        border: `1px solid ${autoApprove ? T.warn : T.border}`,
        background: autoApprove ? T.warnSoft : T.bg,
        color: autoApprove ? T.text : T.textMuted,
        fontSize: 12,
        fontWeight: 600,
        cursor: disabled ? "not-allowed" : "pointer",
        opacity: disabled ? 0.5 : 1,
      }}
    >
      <span
        style={{
          width: 30,
          height: 16,
          borderRadius: 999,
          background: autoApprove ? T.warn : T.border,
          position: "relative",
          transition: "background 200ms ease",
          display: "inline-block",
        }}
      >
        <span
          style={{
            position: "absolute",
            top: 2,
            left: autoApprove ? 16 : 2,
            width: 12,
            height: 12,
            borderRadius: "50%",
            background: "#fff",
            transition: "left 200ms ease",
          }}
        />
      </span>
      Auto-approve {autoApprove ? "ON" : "OFF"}
    </button>
  );
}

// ─── Stage rail (left column) ────────────────────────────────────────

function StageRail({
  stages,
  statuses,
  selectedId,
  onSelect,
}: {
  stages: StageDef[];
  statuses: Record<string, StageStatus>;
  selectedId: string;
  onSelect: (id: string) => void;
}) {
  const phases = useMemo(() => {
    const map = new Map<string, StageDef[]>();
    for (const s of stages) {
      if (!map.has(s.phase)) map.set(s.phase, []);
      map.get(s.phase)!.push(s);
    }
    return Array.from(map.entries());
  }, [stages]);

  return (
    <aside
      style={{
        background: T.bgSidebar,
        borderRight: `1px solid ${T.border}`,
        overflowY: "auto",
        padding: "16px 0",
      }}
    >
      {phases.map(([phase, items]) => (
        <div key={phase} style={{ marginBottom: 16 }}>
          <div
            style={{
              fontSize: 10,
              color: T.textSubtle,
              fontWeight: 700,
              textTransform: "uppercase",
              letterSpacing: 1,
              padding: "4px 20px",
            }}
          >
            {phase}
          </div>
          {items.map((stage) => (
            <StageRow
              key={stage.id}
              stage={stage}
              status={statuses[stage.id] ?? "idle"}
              selected={stage.id === selectedId}
              onClick={() => onSelect(stage.id)}
            />
          ))}
        </div>
      ))}
    </aside>
  );
}

function StageRow({
  stage,
  status,
  selected,
  onClick,
}: {
  stage: StageDef;
  status: StageStatus;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "8px 20px",
        border: "none",
        background: selected ? T.accentSoft : "transparent",
        color: T.text,
        cursor: "pointer",
        width: "100%",
        textAlign: "left",
        fontSize: 13,
        fontWeight: selected ? 600 : 500,
        borderLeft: selected
          ? `3px solid ${T.accent}`
          : "3px solid transparent",
      }}
    >
      <StageIndicator status={status} />
      <span>{stage.label}</span>
    </button>
  );
}

function StageIndicator({ status }: { status: StageStatus }) {
  const base: React.CSSProperties = {
    width: 16,
    height: 16,
    borderRadius: "50%",
    flexShrink: 0,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    fontSize: 10,
    fontWeight: 700,
    color: "#fff",
  };
  if (status === "running") {
    return (
      <div
        style={{
          ...base,
          background: T.bgElev,
          border: `2px solid ${T.accent}`,
          borderTopColor: "transparent",
          animation: "spin 0.9s linear infinite",
        }}
      />
    );
  }
  if (status === "passed") {
    return <div style={{ ...base, background: T.success }}>✓</div>;
  }
  if (status === "failed") {
    return <div style={{ ...base, background: T.error }}>✕</div>;
  }
  return (
    <div
      style={{
        ...base,
        background: T.bgElev,
        border: `2px solid ${T.borderStrong}`,
      }}
    />
  );
}

// ─── Main panel ──────────────────────────────────────────────────────

function MainPanel({
  stage,
  status,
  machine,
  card,
  prNumber,
  autoApprove,
  diff,
  buildFailure,
  resolverStream,
  grpcServerLog,
  onApprove,
  onReject,
  onRetry,
  onRequestChanges,
  onRequestDiff,
  running,
}: {
  stage: StageDef;
  status: StageStatus;
  machine: PrMachine | null;
  card: BoardCard | null;
  prNumber: number;
  autoApprove: boolean;
  diff: string;
  buildFailure: { branch: string; head_sha: string; failed_at: string; error: string } | null;
  resolverStream: string[];
  grpcServerLog: string[];
  running: boolean;
  onApprove: (note?: string) => void;
  onReject: (reason?: string) => void;
  onRetry: () => void;
  onRequestChanges: (feedback: string) => void;
  onRequestDiff: () => void;
}) {
  const steps = card?.steps ?? [];
  const matchingSteps = stepsForStage(stage.id, steps);

  return (
    <main
      style={{
        padding: 24,
        overflowY: "auto",
        minHeight: 0,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 10,
          marginBottom: 16,
        }}
      >
        <StageIndicator status={status} />
        <h2 style={{ margin: 0, fontSize: 16, fontWeight: 700, color: T.text }}>
          {stage.label}
        </h2>
        <StatusChip status={status} />
      </div>

      {/* Retry banner — visible regardless of selected stage when terminal,
          or when the machine is stuck in a non-terminal state while no cycle
          is running (typical ENOSPC / crashed-mid-cycle scenario). */}
      {machine &&
        (machine.status === "failed" ||
          machine.status === "rejected" ||
          (!running && isStuckStatus(machine.status))) && (
          <RetryBanner machine={machine} running={running} onRetry={onRetry} />
        )}

      {stage.id === "approval" ? (
        <ApprovalPanel
          machine={machine}
          autoApprove={autoApprove}
          diff={diff}
          onApprove={onApprove}
          onReject={onReject}
          onRequestChanges={onRequestChanges}
          onRequestDiff={onRequestDiff}
        />
      ) : (
        <DefaultStagePanel
          stage={stage}
          status={status}
          machine={machine}
          steps={matchingSteps}
          prNumber={prNumber}
          buildFailure={buildFailure}
          resolverStream={resolverStream}
          grpcServerLog={grpcServerLog}
        />
      )}

      {/* Event log filtered to this stage at the bottom of the main panel */}
      {matchingSteps.length > 0 && stage.id !== "approval" && (
        <section style={{ marginTop: 24 }}>
          <h3 style={subTitle}>Events</h3>
          <EventList steps={matchingSteps} />
        </section>
      )}

      <section style={{ marginTop: 24 }}>
        <h3 style={subTitle}>Full timeline</h3>
        <EventList steps={steps} dim />
      </section>
    </main>
  );
}

function DefaultStagePanel({
  stage,
  status,
  machine,
  steps,
  prNumber,
  buildFailure,
  resolverStream,
  grpcServerLog,
}: {
  stage: StageDef;
  status: StageStatus;
  machine: PrMachine | null;
  steps: BoardStep[];
  prNumber: number;
  buildFailure: { branch: string; head_sha: string; failed_at: string; error: string } | null;
  resolverStream: string[];
  grpcServerLog: string[];
}) {
  if (stage.id === "notice") {
    return (
      <PanelCard>
        <Field label="PR">#{prNumber}</Field>
        <Field label="Branch">{machine?.branch || "(unknown)"}</Field>
        <Field label="Threads">{machine?.threadIds.length ?? 0}</Field>
        {machine?.startedAt && (
          <Field label="Started">{machine.startedAt}</Field>
        )}
      </PanelCard>
    );
  }
  if (stage.id === "react") {
    return (
      <PanelCard>
        <Field label="Status">
          {status === "passed" ? "👀 posted on each thread" : "Pending"}
        </Field>
        <Field label="Thread count">{machine?.threadIds.length ?? 0}</Field>
      </PanelCard>
    );
  }
  if (stage.id === "resolve") {
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        <PanelCard>
          <Field label="Connectors">
            {(machine?.connectors ?? []).join(", ") || "(none yet)"}
          </Field>
          <Field label="Status">
            {status === "running"
              ? "Claude is resolving comments…"
              : status === "passed"
                ? "Resolver finished — changes staged"
                : status === "failed"
                  ? "Resolver failed"
                  : "Pending"}
          </Field>
          {machine?.summary && (
            <pre style={prePanel}>{machine.summary.slice(0, 4000)}</pre>
          )}
        </PanelCard>
        <ResolverStreamPanel lines={resolverStream} />
      </div>
    );
  }
  if (stage.id === "grpc_test") {
    return (
      <GrpcTestPanel
        machine={machine}
        status={status}
        grpcServerLog={grpcServerLog}
      />
    );
  }
  if (stage.id === "baseline" && buildFailure) {
    return (
      <PanelCard>
        <Field label="Result">Failed at <code style={code}>{buildFailure.head_sha.slice(0, 12)}</code></Field>
        <Field label="Branch">{buildFailure.branch}</Field>
        <Field label="Failed at">{buildFailure.failed_at}</Field>
        <div
          style={{
            fontSize: 11,
            color: T.textMuted,
            marginTop: 4,
            lineHeight: 1.5,
          }}
        >
          The resolver tried to build the PR's HEAD (before applying any of
          its own edits) and cargo errored out. The same SHA is short-circuited
          on later cycles to avoid retrying a known-broken build. <strong>Retry
          clears this cache</strong> so the build runs fresh.
        </div>
        <pre style={{ ...prePanel, color: T.error, background: T.errorSoft }}>
          {buildFailure.error || "(no error output captured)"}
        </pre>
      </PanelCard>
    );
  }
  if (steps.length === 0) {
    return (
      <PanelCard>
        <div
          style={{
            fontSize: 12,
            color: T.textSubtle,
            fontStyle: "italic",
          }}
        >
          No events captured for this stage yet.
        </div>
      </PanelCard>
    );
  }
  // Default: show the latest matching step's detail.
  const latest = steps[steps.length - 1]!;
  return (
    <PanelCard>
      <Field label="Latest">{latest.text}</Field>
      {latest.detail && <pre style={prePanel}>{latest.detail}</pre>}
    </PanelCard>
  );
}

function ResolverStreamPanel({ lines }: { lines: string[] }) {
  // Auto-scroll to bottom on new lines.
  const scrollRef = useRef<HTMLPreElement | null>(null);
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [lines]);

  return (
    <PanelCard>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 8,
        }}
      >
        <h3 style={{ ...subTitle, margin: 0 }}>Claude live output</h3>
        <span style={{ fontSize: 10, color: T.textMuted }}>
          {lines.length === 0
            ? "no output yet"
            : `${lines.length} line${lines.length === 1 ? "" : "s"} · last ~300 kept`}
        </span>
      </div>
      {lines.length === 0 ? (
        <div
          style={{
            fontSize: 12,
            color: T.textSubtle,
            fontStyle: "italic",
            padding: 12,
          }}
        >
          Streamed from <code style={code}>claude --verbose</code> stdout while
          the resolver session is running.
        </div>
      ) : (
        <pre
          ref={scrollRef}
          style={{
            margin: 0,
            padding: 12,
            background: "#1f1810",
            color: "#f0e0c4",
            borderRadius: 6,
            fontSize: 11,
            lineHeight: 1.45,
            fontFamily:
              "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
            maxHeight: 360,
            overflowY: "auto",
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
          }}
        >
          {lines.join("\n")}
        </pre>
      )}
    </PanelCard>
  );
}

function RetryBanner({
  machine,
  running,
  onRetry,
}: {
  machine: PrMachine;
  running: boolean;
  onRetry: () => void;
}) {
  const isRejected = machine.status === "rejected";
  const isStuck = !running && isStuckStatus(machine.status);
  const tone = isStuck ? "stuck" : isRejected ? "rejected" : "failed";
  const palette = {
    stuck: { bg: T.warnSoft, border: T.warn + "55" },
    rejected: { bg: T.warnSoft, border: T.warn + "55" },
    failed: { bg: T.errorSoft, border: T.error + "55" },
  }[tone];
  const heading = isStuck
    ? `Stuck mid-cycle in '${machine.status}'`
    : isRejected
      ? "Rejected"
      : "Failed";
  const detail = isStuck
    ? `The resolver isn't running, so this PR isn't going to move on its own. ` +
      `Most often this is a disk-space / crashed-cycle situation — fix the ` +
      `underlying cause, then click Retry to clear state and requeue.`
    : `Retry clears this PR's thread + machine state so the next poll cycle picks it up fresh.`;
  return (
    <div
      data-testid={`retry-banner-${tone}`}
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: 12,
        padding: 12,
        marginBottom: 16,
        background: palette.bg,
        border: `1px solid ${palette.border}`,
        borderRadius: 8,
      }}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
        <span style={{ fontSize: 13, fontWeight: 600, color: T.text }}>
          {heading}
          {machine.reason ? `: ${machine.reason}` : ""}
        </span>
        <span style={{ fontSize: 11, color: T.textMuted, lineHeight: 1.5 }}>
          {detail}
        </span>
      </div>
      <button
        type="button"
        onClick={onRetry}
        style={{
          padding: "8px 14px",
          borderRadius: 6,
          border: "none",
          background: T.accent,
          color: "#fff",
          fontWeight: 600,
          fontSize: 12,
          cursor: "pointer",
          whiteSpace: "nowrap",
        }}
      >
        {isStuck ? "Force reset & retry" : "Retry this PR"}
      </button>
    </div>
  );
}

function isStuckStatus(status: PrMachineStatus): boolean {
  return (
    status === "noticed" ||
    status === "preparing" ||
    status === "resolving" ||
    status === "verifying" ||
    status === "committing"
  );
}

function GrpcTestPanel({
  machine,
  status,
  grpcServerLog,
}: {
  machine: PrMachine | null;
  status: StageStatus;
  grpcServerLog: string[];
}) {
  if (!machine) {
    return (
      <PanelCard>
        <div style={{ fontSize: 12, color: T.textMuted }}>
          No machine yet — resolver hasn't reached this stage.
        </div>
      </PanelCard>
    );
  }
  const stepResults: GrpcTestStepResult[] = machine.testStepResults ?? [];
  const plan = machine.testPlan;
  const planSteps = Array.isArray(plan?.tests) ? (plan!.tests as unknown[]) : [];
  const legacyCommands = machine.testCommands ?? [];
  const legacyResults = machine.testResults ?? [];
  const generationReply = machine.testGenerationReply ?? "";
  const usingPlan = planSteps.length > 0 || stepResults.length > 0;
  const failed = stepResults.filter((r) => !r.ok && !r.skipped).length;
  const skipped = stepResults.filter((r) => r.skipped).length;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <PanelCard>
        <Field label="Status">
          {status === "running"
            ? "Running test plan…"
            : status === "passed"
              ? `All steps passed (${stepResults.length || legacyResults.length})`
              : status === "failed"
                ? `Failed (${failed} failed, ${skipped} skipped)`
                : "Pending"}
        </Field>
        <Field label="Source">
          {usingPlan
            ? "Claude — single call with PR body + comments + diff + creds"
            : machine.testCommandsSource === "extracted"
              ? "Legacy: extracted commands from PR body"
              : machine.testCommandsSource === "generated"
                ? "Legacy: bash commands generated by Claude"
                : "None — step was skipped"}
        </Field>
        <Field label="Steps">
          {usingPlan
            ? `${planSteps.length || stepResults.length}`
            : `${legacyCommands.length || "(none)"}`}
        </Field>
      </PanelCard>
      {usingPlan && (
        <PanelCard>
          <h3 style={{ ...subTitle, margin: "0 0 8px" }}>Test plan</h3>
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {stepResults.length > 0
              ? stepResults.map((r, i) => (
                  <PlanStepRow key={r.name + i} step={r} />
                ))
              : planSteps.map((s, i) => (
                  <PlanStepPreview key={i} step={s} index={i + 1} />
                ))}
          </div>
        </PanelCard>
      )}
      {!usingPlan && legacyCommands.length > 0 && (
        <PanelCard>
          <h3 style={{ ...subTitle, margin: "0 0 8px" }}>Legacy commands</h3>
          <ol style={{ margin: 0, paddingLeft: 18, fontSize: 12, color: T.text, lineHeight: 1.6 }}>
            {legacyCommands.map((cmd, i) => (
              <li key={i}>
                <code style={code}>{cmd}</code>
              </li>
            ))}
          </ol>
        </PanelCard>
      )}
      {!usingPlan && legacyResults.length > 0 && (
        <PanelCard>
          <h3 style={{ ...subTitle, margin: "0 0 8px" }}>Legacy results</h3>
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {legacyResults.map((r, i) => (
              <GrpcResultRow key={i} index={i + 1} result={r} />
            ))}
          </div>
        </PanelCard>
      )}
      {!usingPlan && generationReply && (
        <PanelCard>
          <h3 style={{ ...subTitle, margin: "0 0 8px" }}>
            Claude reply (no plan parsed)
          </h3>
          <div style={{ fontSize: 11, color: T.textMuted, marginBottom: 6 }}>
            The test-plan parser couldn't extract a valid JSON plan from this
            response. Paste-able for debugging:
          </div>
          <pre style={prePanel}>{generationReply.slice(0, 8_000)}</pre>
        </PanelCard>
      )}
      {grpcServerLog.length > 0 && (
        <ServerLogPanel lines={grpcServerLog} />
      )}
    </div>
  );
}

function ServerLogPanel({ lines }: { lines: string[] }) {
  const scrollRef = useRef<HTMLPreElement | null>(null);
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [lines]);
  return (
    <PanelCard>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 8,
        }}
      >
        <h3 style={{ ...subTitle, margin: 0 }}>Server live output</h3>
        <span style={{ fontSize: 10, color: T.textMuted }}>
          {lines.length} line{lines.length === 1 ? "" : "s"} · `cargo run -p grpc-server`
        </span>
      </div>
      <pre
        ref={scrollRef}
        style={{
          margin: 0,
          padding: 12,
          background: "#1f1810",
          color: "#f0e0c4",
          borderRadius: 6,
          fontSize: 11,
          lineHeight: 1.45,
          fontFamily:
            "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
          maxHeight: 360,
          overflowY: "auto",
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
        }}
      >
        {lines.join("\n")}
      </pre>
    </PanelCard>
  );
}

function PlanStepPreview({
  step,
  index,
}: {
  step: unknown;
  index: number;
}) {
  const s = (step ?? {}) as {
    name?: string;
    method?: string;
    depends_on?: string;
  };
  return (
    <div
      style={{
        padding: 10,
        borderRadius: 6,
        background: T.bgElev,
        border: `1px solid ${T.border}`,
      }}
    >
      <div style={{ fontSize: 12, color: T.text, marginBottom: 4 }}>
        <span style={{ color: T.textMuted, marginRight: 6 }}>#{index}</span>
        <strong>{s.name ?? "(unnamed)"}</strong>
        {s.depends_on && (
          <span style={{ color: T.textMuted, marginLeft: 8, fontSize: 11 }}>
            depends on {s.depends_on}
          </span>
        )}
      </div>
      <code style={{ ...code, fontSize: 11 }}>{s.method ?? "?"}</code>
    </div>
  );
}

function PlanStepRow({ step }: { step: GrpcTestStepResult }) {
  const [expanded, setExpanded] = useState(false);
  const colors = step.skipped
    ? { bg: T.warnSoft, border: T.warn + "55", icon: "↷", iconColor: T.warn }
    : step.ok
      ? { bg: T.successSoft, border: T.success + "55", icon: "✓", iconColor: T.success }
      : { bg: T.errorSoft, border: T.error + "55", icon: "✕", iconColor: T.error };
  return (
    <div
      style={{
        padding: 10,
        borderRadius: 6,
        background: colors.bg,
        border: `1px solid ${colors.border}`,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          fontSize: 12,
          cursor: "pointer",
        }}
        onClick={() => setExpanded((v) => !v)}
      >
        <span style={{ color: colors.iconColor, fontWeight: 700 }}>
          {colors.icon}
        </span>
        <strong style={{ color: T.text }}>{step.name}</strong>
        <code style={{ ...code, fontSize: 10 }}>{step.method}</code>
        <span style={{ color: T.textMuted, fontSize: 11, marginLeft: "auto" }}>
          {step.skipped
            ? "skipped"
            : `exit=${step.exitCode ?? "?"} · ${step.durationMs}ms`}
        </span>
        <span style={{ color: T.textSubtle, fontSize: 10 }}>
          {expanded ? "▾" : "▸"}
        </span>
      </div>
      {step.skipped && step.skipReason && (
        <div style={{ fontSize: 11, color: T.textMuted, marginTop: 4 }}>
          {step.skipReason}
        </div>
      )}
      {step.expectMisses.length > 0 && (
        <ul
          style={{
            margin: "6px 0 0 16px",
            padding: 0,
            fontSize: 11,
            color: T.error,
          }}
        >
          {step.expectMisses.map((m, i) => (
            <li key={i}>{m}</li>
          ))}
        </ul>
      )}
      {expanded && !step.skipped && (
        <div style={{ marginTop: 8 }}>
          {Object.keys(step.captures).length > 0 && (
            <>
              <div style={miniLabel}>Captures</div>
              <pre style={prePanel}>
                {JSON.stringify(step.captures, null, 2)}
              </pre>
            </>
          )}
          <div style={miniLabel}>Command</div>
          <pre style={prePanel}>{step.command}</pre>
          {step.stdout && (
            <>
              <div style={miniLabel}>stdout</div>
              <pre style={prePanel}>{step.stdout}</pre>
            </>
          )}
          {step.stderr && (
            <>
              <div style={miniLabel}>stderr</div>
              <pre style={{ ...prePanel, color: T.error }}>{step.stderr}</pre>
            </>
          )}
        </div>
      )}
    </div>
  );
}

const miniLabel: React.CSSProperties = {
  fontSize: 10,
  fontWeight: 600,
  color: T.textMuted,
  textTransform: "uppercase",
  letterSpacing: 0.5,
  marginTop: 8,
  marginBottom: 4,
};

function GrpcResultRow({
  index,
  result,
}: {
  index: number;
  result: GrpcTestResultRecord;
}) {
  const passed = result.ok;
  return (
    <div
      style={{
        padding: 10,
        borderRadius: 6,
        background: passed ? T.successSoft : T.errorSoft,
        border: `1px solid ${passed ? T.success + "55" : T.error + "55"}`,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 6,
          fontSize: 12,
        }}
      >
        <span style={{ fontWeight: 600, color: passed ? T.success : T.error }}>
          {passed ? "✓" : "✕"} #{index} · exit={result.exitCode ?? "?"}
          {result.timedOut ? " · timed out" : ""}
        </span>
        <span style={{ color: T.textMuted, fontSize: 11 }}>
          {result.durationMs}ms
        </span>
      </div>
      <code
        style={{
          ...code,
          display: "block",
          padding: 6,
          marginBottom: 6,
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
        }}
      >
        {result.command}
      </code>
      {result.stdout && (
        <pre style={prePanel}>{result.stdout}</pre>
      )}
      {result.stderr && (
        <pre
          style={{
            ...prePanel,
            background: T.codeBg,
            color: T.error,
            marginTop: 6,
          }}
        >
          {result.stderr}
        </pre>
      )}
    </div>
  );
}

function ReviewerSummaryElapsed({ startedAt }: { startedAt: number }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);
  const secs = Math.max(0, Math.floor((now - startedAt) / 1000));
  return <span>{secs}s</span>;
}

function ReviewerSummaryPanel({ machine }: { machine: PrMachine }) {
  // Use the machine's `updatedAt` as the start anchor for the elapsed counter.
  // It's set when reviewSummaryStatus flipped to "generating", which is what
  // we want to measure from. Cheap and avoids carrying a separate field.
  const startedAt = Date.parse(machine.updatedAt) || Date.now();
  const status = machine.reviewSummaryStatus;
  if (!status && !machine.reviewSummary) {
    return null;
  }
  return (
    <PanelCard>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 10,
          marginBottom: 12,
        }}
      >
        <h3 style={{ ...subTitle, margin: 0 }}>What changed & why</h3>
        {status === "generating" && (
          <span
            data-testid="reviewer-summary-generating"
            style={{
              fontSize: 11,
              color: T.textMuted,
              display: "inline-flex",
              alignItems: "center",
              gap: 6,
            }}
          >
            <span
              style={{
                width: 8,
                height: 8,
                borderRadius: "50%",
                background: T.accent,
                animation: "prResolverPulse 1.2s infinite",
                display: "inline-block",
              }}
            />
            Generating… <ReviewerSummaryElapsed startedAt={startedAt} />
          </span>
        )}
      </div>
      {status === "generating" && (
        <div
          style={{
            fontSize: 12,
            color: T.textMuted,
            lineHeight: 1.5,
            padding: "10px 12px",
            background: T.bg,
            border: `1px dashed ${T.border}`,
            borderRadius: 6,
          }}
        >
          A plain-language summary is being generated so you can decide
          quickly. Approve / reject any time — the diff below is the source
          of truth.
        </div>
      )}
      {status === "ready" && machine.reviewSummary && (
        <div
          data-testid="reviewer-summary-ready"
          style={{
            fontSize: 13,
            lineHeight: 1.55,
            color: T.text,
            maxHeight: 480,
            overflow: "auto",
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
            padding: "12px 14px",
            background: T.bg,
            border: `1px solid ${T.border}`,
            borderRadius: 6,
            fontFamily:
              "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
          }}
        >
          {machine.reviewSummary}
        </div>
      )}
      {status === "failed" && (
        <div
          data-testid="reviewer-summary-failed"
          style={{
            fontSize: 12,
            color: T.text,
            lineHeight: 1.5,
            padding: "10px 12px",
            background: T.warnSoft,
            border: `1px solid ${T.warn}55`,
            borderRadius: 6,
          }}
        >
          <strong>Couldn't generate summary.</strong> Review the diff
          directly.
          {machine.reviewSummaryError && (
            <div
              style={{
                marginTop: 6,
                color: T.textMuted,
                fontFamily: "monospace",
                fontSize: 11,
                whiteSpace: "pre-wrap",
                wordBreak: "break-word",
              }}
            >
              {machine.reviewSummaryError}
            </div>
          )}
        </div>
      )}
    </PanelCard>
  );
}

function ApprovalPanel({
  machine,
  autoApprove,
  diff,
  onApprove,
  onReject,
  onRequestChanges,
  onRequestDiff,
}: {
  machine: PrMachine | null;
  autoApprove: boolean;
  diff: string;
  onApprove: (note?: string) => void;
  onReject: (reason?: string) => void;
  onRequestChanges: (feedback: string) => void;
  onRequestDiff: () => void;
}) {
  const [rejectReason, setRejectReason] = useState("");
  const [confirmReject, setConfirmReject] = useState(false);
  const [revisionFeedback, setRevisionFeedback] = useState("");
  const [showRevisionForm, setShowRevisionForm] = useState(false);

  if (autoApprove && machine?.status !== "awaiting_approval") {
    return (
      <PanelCard>
        <Field label="Auto-approve">ON</Field>
        <div style={{ fontSize: 12, color: T.textMuted, lineHeight: 1.5 }}>
          Auto-approve is on for this session. Once the build + clippy pass, the
          resolver pushes commits automatically without waiting on you.
        </div>
      </PanelCard>
    );
  }

  if (!machine) {
    return (
      <PanelCard>
        <div style={{ fontSize: 12, color: T.textMuted }}>
          No machine yet — the resolver hasn't reached this stage.
        </div>
      </PanelCard>
    );
  }

  if (machine.status === "pushed") {
    return (
      <PanelCard>
        <Field label="Status">Approved & pushed</Field>
        {machine.localSha && (
          <Field label="Commit">
            <code style={code}>{machine.localSha.slice(0, 12)}</code>
          </Field>
        )}
      </PanelCard>
    );
  }
  if (machine.status === "rejected") {
    return (
      <PanelCard>
        <Field label="Status">Rejected</Field>
        {machine.reason && <Field label="Reason">{machine.reason}</Field>}
      </PanelCard>
    );
  }
  if (machine.status === "failed") {
    return (
      <PanelCard>
        <Field label="Status">Failed before approval</Field>
        {machine.reason && <Field label="Reason">{machine.reason}</Field>}
      </PanelCard>
    );
  }
  if (machine.status !== "awaiting_approval") {
    return (
      <PanelCard>
        <div style={{ fontSize: 12, color: T.textMuted, lineHeight: 1.5 }}>
          Approval becomes actionable once the resolver finishes the cargo
          fix loop. Current status: <strong>{machine.status}</strong>.
        </div>
      </PanelCard>
    );
  }

  // Awaiting approval — the meat of this panel.
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <PanelCard>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            marginBottom: 8,
          }}
        >
          <span
            style={{
              width: 10,
              height: 10,
              borderRadius: "50%",
              background: T.warn,
              animation: "prResolverPulse 1.5s infinite",
              display: "inline-block",
            }}
          />
          <strong>Awaiting your approval</strong>
        </div>
        <div style={{ fontSize: 12, color: T.textMuted, lineHeight: 1.5 }}>
          The resolver has local commits ready to push. Review the diff below,
          then approve to <code style={code}>git push</code> (fast-forward only)
          or reject to <code style={code}>git reset --hard origin/{machine.branch}</code>{" "}
          and post a rejection reply on each thread.
        </div>
        <div style={{ marginTop: 12, display: "flex", gap: 10, flexWrap: "wrap" }}>
          <button
            type="button"
            onClick={() => onApprove()}
            style={buttonPrimary}
          >
            Approve & push
          </button>
          <button
            type="button"
            onClick={() => {
              setShowRevisionForm((v) => !v);
              setConfirmReject(false);
            }}
            style={buttonSecondary}
          >
            Request changes…
          </button>
          <button
            type="button"
            onClick={() => {
              setConfirmReject((v) => !v);
              setShowRevisionForm(false);
            }}
            style={buttonSecondary}
          >
            Reject…
          </button>
          {!diff && (
            <button
              type="button"
              onClick={onRequestDiff}
              style={{ ...buttonSecondary, marginLeft: "auto" }}
            >
              Reload diff
            </button>
          )}
        </div>
        {showRevisionForm && (
          <div
            data-testid="revision-form"
            style={{
              marginTop: 12,
              padding: 12,
              background: T.warnSoft,
              borderRadius: 6,
              border: `1px solid ${T.warn}55`,
            }}
          >
            <Field label="What should Claude change? (sent as the overriding instruction to the next resolve loop)">
              <textarea
                placeholder="e.g. use Option<String> for the optional fields, and pass through the raw status code on the error path"
                value={revisionFeedback}
                onChange={(e) => setRevisionFeedback(e.target.value)}
                rows={4}
                style={{
                  ...inputStyle,
                  resize: "vertical",
                  fontFamily:
                    "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
                  lineHeight: 1.5,
                }}
              />
            </Field>
            <div
              style={{
                marginTop: 8,
                fontSize: 11,
                color: T.textMuted,
                lineHeight: 1.5,
              }}
            >
              The local commits will be reset; Claude re-runs the resolve + cargo
              + gRPC loop using this feedback as the highest-priority instruction.
              You'll see a fresh diff to review when it lands back in approval.
            </div>
            <div style={{ marginTop: 8, display: "flex", gap: 8 }}>
              <button
                type="button"
                disabled={!revisionFeedback.trim()}
                onClick={() => {
                  onRequestChanges(revisionFeedback.trim());
                  setRevisionFeedback("");
                  setShowRevisionForm(false);
                }}
                style={
                  revisionFeedback.trim()
                    ? buttonPrimary
                    : { ...buttonPrimary, opacity: 0.5, cursor: "not-allowed" }
                }
              >
                Send to Claude
              </button>
              <button
                type="button"
                onClick={() => {
                  setShowRevisionForm(false);
                  setRevisionFeedback("");
                }}
                style={buttonSecondary}
              >
                Cancel
              </button>
            </div>
          </div>
        )}
        {confirmReject && (
          <div
            style={{
              marginTop: 12,
              padding: 12,
              background: T.errorSoft,
              borderRadius: 6,
              border: `1px solid ${T.error}33`,
            }}
          >
            <Field label="Reason (optional — posted to GitHub)">
              <input
                type="text"
                placeholder="e.g. wrong field name, please use Option<String>"
                value={rejectReason}
                onChange={(e) => setRejectReason(e.target.value)}
                style={inputStyle}
              />
            </Field>
            <div style={{ marginTop: 8, display: "flex", gap: 8 }}>
              <button
                type="button"
                onClick={() => {
                  onReject(rejectReason);
                  setRejectReason("");
                  setConfirmReject(false);
                }}
                style={buttonDanger}
              >
                Confirm reject
              </button>
              <button
                type="button"
                onClick={() => setConfirmReject(false)}
                style={buttonSecondary}
              >
                Cancel
              </button>
            </div>
          </div>
        )}
      </PanelCard>
      <ReviewerSummaryPanel machine={machine} />
      {machine.summary && machine.summary.trim() && (
        <PanelCard>
          <h3 style={{ ...subTitle, margin: "0 0 12px" }}>
            Per-thread resolve notes
          </h3>
          <div
            data-testid="approval-summary"
            style={{
              fontSize: 13,
              lineHeight: 1.55,
              color: T.text,
              maxHeight: 320,
              overflow: "auto",
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
              padding: "10px 12px",
              background: T.bg,
              border: `1px solid ${T.border}`,
              borderRadius: 6,
              fontFamily:
                "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
            }}
          >
            {machine.summary}
          </div>
        </PanelCard>
      )}
      <PanelCard>
        <h3 style={{ ...subTitle, margin: "0 0 12px" }}>Diff</h3>
        <DiffViewer diff={diff} collapsedByDefault={diff.split("\n").length > 200} />
      </PanelCard>
    </div>
  );
}

function EventList({ steps, dim }: { steps: BoardStep[]; dim?: boolean }) {
  const sorted = [...steps].sort((a, b) => a.timestamp - b.timestamp);
  return (
    <ol
      style={{
        listStyle: "none",
        margin: 0,
        padding: 0,
        display: "flex",
        flexDirection: "column",
        gap: 4,
        opacity: dim ? 0.85 : 1,
      }}
    >
      {sorted.map((step, i) => (
        <li
          key={i}
          style={{
            display: "grid",
            gridTemplateColumns: "82px 16px 1fr",
            gap: 8,
            alignItems: "baseline",
            fontSize: 12,
            padding: "4px 8px",
            borderRadius: 4,
            background: i % 2 === 0 ? T.bg : "transparent",
          }}
        >
          <span style={{ color: T.textSubtle, fontFamily: "monospace", fontSize: 10 }}>
            {formatTime(step.timestamp)}
          </span>
          <span
            style={{
              color: stepIconColor(step),
              fontWeight: 700,
              textAlign: "center",
            }}
          >
            {stepIcon(step)}
          </span>
          <div style={{ minWidth: 0 }}>
            <span style={{ color: T.text }}>{step.text}</span>
            {step.detail && (
              <div
                style={{
                  color: T.textMuted,
                  fontFamily: "monospace",
                  fontSize: 11,
                  marginTop: 2,
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-word",
                }}
              >
                {step.detail}
              </div>
            )}
          </div>
        </li>
      ))}
    </ol>
  );
}

function HistorySection({
  threads,
}: {
  threads: Array<{ threadId: string } & Record<string, unknown>>;
}) {
  return (
    <section
      style={{
        background: T.bgElev,
        border: `1px solid ${T.border}`,
        borderRadius: 10,
        padding: 18,
        margin: "0 24px 32px",
        boxShadow: T.shadow,
      }}
    >
      <h2 style={subTitle}>Thread history (from state.json)</h2>
      <ul
        style={{
          listStyle: "none",
          padding: 0,
          margin: 0,
          display: "flex",
          flexDirection: "column",
          gap: 10,
        }}
      >
        {threads.map((t, i) => (
          <li
            key={t.threadId + i}
            style={{
              padding: 12,
              borderRadius: 6,
              background: T.bg,
              border: `1px solid ${T.border}`,
              fontSize: 12,
              color: T.text,
            }}
          >
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                marginBottom: 4,
              }}
            >
              <code style={code}>{t.threadId}</code>
              <code style={code}>{String(t.status)}</code>
            </div>
            {t.path !== undefined && (
              <div style={{ fontSize: 11, color: T.textMuted }}>
                {String(t.path)}
              </div>
            )}
            {t.instruction_preview !== undefined && (
              <div style={{ fontSize: 11, color: T.textMuted, marginTop: 4 }}>
                {String(t.instruction_preview)}
              </div>
            )}
            {t.resolution_summary !== undefined && t.resolution_summary !== "" && (
              <div
                style={{
                  marginTop: 6,
                  padding: 6,
                  background: T.successSoft,
                  borderRadius: 4,
                  fontSize: 11,
                  whiteSpace: "pre-wrap",
                }}
              >
                {String(t.resolution_summary)}
              </div>
            )}
            {t.error !== undefined && t.error !== "" && (
              <div
                style={{
                  marginTop: 6,
                  padding: 6,
                  background: T.errorSoft,
                  color: T.error,
                  borderRadius: 4,
                  fontSize: 11,
                  whiteSpace: "pre-wrap",
                  fontFamily: "monospace",
                }}
              >
                {String(t.error)}
              </div>
            )}
            {t.commit_sha !== undefined && t.commit_sha !== "" && (
              <div style={{ marginTop: 4, fontSize: 10, color: T.textSubtle }}>
                commit <code style={code}>{String(t.commit_sha).slice(0, 8)}</code>
              </div>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

// ─── Pieces ──────────────────────────────────────────────────────────

function PanelCard({ children }: { children: React.ReactNode }) {
  return (
    <section
      style={{
        background: T.bgElev,
        border: `1px solid ${T.border}`,
        borderRadius: 10,
        padding: 16,
        boxShadow: T.shadow,
        display: "flex",
        flexDirection: "column",
        gap: 10,
      }}
    >
      {children}
    </section>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
      <span style={{ fontSize: 10, color: T.textMuted, fontWeight: 600 }}>
        {label}
      </span>
      <span style={{ fontSize: 13, color: T.text }}>{children}</span>
    </div>
  );
}

function StatusChip({ status }: { status: StageStatus }) {
  const color =
    status === "passed"
      ? T.success
      : status === "failed"
        ? T.error
        : status === "running"
          ? "#3b82f6"
          : T.textMuted;
  return (
    <span
      style={{
        fontSize: 10,
        textTransform: "uppercase",
        letterSpacing: 0.5,
        color,
        fontWeight: 700,
      }}
    >
      {status}
    </span>
  );
}

function ConnDot({ status }: { status: "connecting" | "open" | "closed" }) {
  const color =
    status === "open" ? T.success : status === "connecting" ? T.warn : T.error;
  const label =
    status === "open"
      ? "Live"
      : status === "connecting"
        ? "Connecting…"
        : "Disconnected";
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        fontSize: 12,
        color: T.textMuted,
      }}
    >
      <span
        style={{
          width: 8,
          height: 8,
          borderRadius: "50%",
          background: color,
          animation:
            status === "open" ? "prResolverPulse 2s infinite" : undefined,
        }}
      />
      {label}
    </span>
  );
}

function StatusPill({
  label,
  color,
  pulse,
}: {
  label: string;
  color: string;
  pulse?: boolean;
}) {
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        fontSize: 12,
        color: T.text,
        background: T.bgElev,
        border: `1px solid ${T.border}`,
        padding: "4px 10px",
        borderRadius: 12,
      }}
    >
      <span
        style={{
          width: 8,
          height: 8,
          borderRadius: "50%",
          background: color,
          animation: pulse ? "prResolverPulse 1.5s infinite" : undefined,
        }}
      />
      {label}
    </span>
  );
}

function NotFound({ message }: { message: string }) {
  return (
    <div
      style={{
        margin: "24px 32px 0",
        padding: 16,
        background: T.warnSoft,
        borderRadius: 8,
        color: T.text,
        fontSize: 13,
      }}
    >
      {message}
    </div>
  );
}

// ─── Helpers ─────────────────────────────────────────────────────────

function computeStageStatuses(
  machine: PrMachine | null,
  steps: BoardStep[]
): Record<string, StageStatus> {
  const out: Record<string, StageStatus> = {};
  for (const stage of STAGES) {
    out[stage.id] = "idle";
  }
  if (!machine) return out;

  const machinePhase = MACHINE_PHASE_ORDER[machine.status];

  // Map machine status → which stages are at-most "running".
  // Past stages are passed by default unless a failed step contradicts.
  const stageMachineFloor: Record<string, number> = {
    notice: 0, // any non-null machine is past noticed
    react: 0,
    pr_open: 1,
    checkout: 1,
    baseline: 1,
    threads: 1,
    resolve: 2,
    build: 3,
    clippy: 3,
    grpc_test: 3,
    fmt: 3,
    scope: 3,
    approval: 4,
    commit: 5,
    push: 5,
    reply: 5,
  };

  for (const stage of STAGES) {
    const floor = stageMachineFloor[stage.id] ?? 0;
    if (machinePhase > floor) {
      out[stage.id] = "passed";
    } else if (machinePhase === floor) {
      out[stage.id] = "running";
    }
  }

  // Refine with step data — a failed step trumps a passed-by-phase guess.
  for (const step of steps) {
    const stageId = stageIdFromStep(step);
    if (!stageId) continue;
    if (step.passed === false) out[stageId] = "failed";
    else if (step.passed === true && out[stageId] !== "failed") {
      out[stageId] = "passed";
    }
  }

  // gRPC verification: machine.testStepResults is authoritative. If any
  // step failed, the stage is failed regardless of what individual step
  // events flow through.
  if (machine.testStepResults && machine.testStepResults.length > 0) {
    const failedCount = machine.testStepResults.filter(
      (r) => !r.ok && !r.skipped
    ).length;
    out["grpc_test"] = failedCount > 0 ? "failed" : "passed";
  }

  // Approval / rejected / failed terminal statuses
  if (machine.status === "awaiting_approval") {
    out["approval"] = "running";
  }
  if (machine.status === "rejected") {
    out["approval"] = "failed";
  }
  if (machine.status === "pushed") {
    out["approval"] = "passed";
    out["commit"] = "passed";
    out["push"] = "passed";
    out["reply"] = "passed";
  }
  if (machine.status === "failed") {
    // Terminal failure: the phase-floor logic above marks everything ≤ phase
    // as "passed", but `failed` shares phase 6 with `pushed`, so every stage
    // ends up green. Reset to "idle" first, then derive from actual step /
    // testStepResults evidence.
    for (const stage of STAGES) {
      // Skip stages that already have explicit evidence from steps /
      // testStepResults — those reads from the refinement above are
      // authoritative.
      const hasExplicit = steps.some(
        (s) => stageIdFromStep(s) === stage.id && s.passed !== undefined
      );
      const hasGrpcEvidence =
        stage.id === "grpc_test" &&
        machine.testStepResults &&
        machine.testStepResults.length > 0;
      if (!hasExplicit && !hasGrpcEvidence) {
        out[stage.id] = "idle";
      }
    }
    // Mark the EARLIEST stage with a failure as failed. If we have a
    // failing stage from steps/testStepResults, that's the failure point;
    // everything before it that lacks explicit passing evidence stays idle.
    const failedStage = STAGES.find((s) => out[s.id] === "failed");
    // If somehow no stage was marked failed but the machine says failed,
    // walk forward through stages and mark the first non-passed one.
    if (!failedStage) {
      for (const stage of STAGES) {
        if (out[stage.id] !== "passed") {
          out[stage.id] = "failed";
          break;
        }
      }
    }
  }

  return out;
}

function stageIdFromStep(step: BoardStep): string | null {
  // Prefer the structured `type` over text matching. The grpc_test_step_*
  // events emit text like "✗ authorize_manual_capture" which has no "grpc"
  // substring; text matching would miss them and the failed stage wouldn't
  // be marked red on the left rail.
  if (step.type === "grpc_test") return "grpc_test";
  if (step.type === "push") return "push";
  if (step.type === "reply") return "reply";
  if (step.type === "subtask" || step.type === "pr_start") return "resolve";
  // review_summary doesn't belong to any verification stage — it lives in
  // the Approval panel itself.
  if (step.type === "review_summary") return null;

  const t = step.text.toLowerCase();
  if (t.includes("pr still open")) return "pr_open";
  if (t.includes("checkout branch")) return "checkout";
  if (t.includes("baseline build")) return "baseline";
  if (t.includes("threads unresolved")) return "threads";
  if (t.includes("build pass") || t.includes("build fail")) return "build";
  if (t.includes("clippy pass") || t.includes("clippy fail")) return "clippy";
  if (t.includes("grpc")) return "grpc_test";
  if (t.includes("format")) return "fmt";
  if (t.includes("scope")) return "scope";
  if (t.includes("committed")) return "commit";
  if (t.includes("pushed")) return "push";
  if (t.includes("reply")) return "reply";
  if (t.includes("started") || t.includes("sub-task")) return "resolve";
  return null;
}

function stepsForStage(stageId: string, steps: BoardStep[]): BoardStep[] {
  return steps.filter((s) => stageIdFromStep(s) === stageId);
}

function machineStatusLabel(status: PrMachineStatus | null): {
  label: string;
  color: string;
} {
  switch (status) {
    case "noticed":
      return { label: "Noticed", color: "#9ca3af" };
    case "preparing":
      return { label: "Preparing", color: "#3b82f6" };
    case "resolving":
      return { label: "Resolving", color: "#3b82f6" };
    case "verifying":
      return { label: "Verifying", color: "#3b82f6" };
    case "awaiting_approval":
      return { label: "Awaiting approval", color: T.warn };
    case "committing":
      return { label: "Pushing", color: "#3b82f6" };
    case "pushed":
      return { label: "Pushed", color: T.success };
    case "rejected":
      return { label: "Rejected", color: T.error };
    case "failed":
      return { label: "Failed", color: T.error };
    default:
      return { label: "Unknown", color: T.textMuted };
  }
}

function stepIcon(step: BoardStep): string {
  if (step.passed === true) return "✓";
  if (step.passed === false) return "✕";
  if (step.type === "push" || step.type === "reply") return "→";
  if (step.type === "cargo") return "⚙";
  return "•";
}

function stepIconColor(step: BoardStep): string {
  if (step.passed === true) return T.success;
  if (step.passed === false) return T.error;
  return T.textMuted;
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  return `${hh}:${mm}:${ss}`;
}

const subTitle: React.CSSProperties = {
  margin: "0 0 8px",
  fontSize: 12,
  fontWeight: 700,
  color: T.text,
  textTransform: "uppercase",
  letterSpacing: 0.4,
};

const prePanel: React.CSSProperties = {
  margin: 0,
  padding: 12,
  background: T.bg,
  borderRadius: 6,
  border: `1px solid ${T.border}`,
  fontSize: 11,
  lineHeight: 1.4,
  whiteSpace: "pre-wrap",
  wordBreak: "break-word",
  maxHeight: 320,
  overflowY: "auto",
};

const inputStyle: React.CSSProperties = {
  padding: "8px 10px",
  borderRadius: 6,
  border: `1px solid ${T.border}`,
  background: T.bg,
  color: T.text,
  fontSize: 13,
  width: "100%",
  outline: "none",
};

const buttonPrimary: React.CSSProperties = {
  padding: "8px 16px",
  borderRadius: 6,
  border: "none",
  background: T.accent,
  color: "#fff",
  fontWeight: 600,
  fontSize: 13,
  cursor: "pointer",
};

const buttonSecondary: React.CSSProperties = {
  padding: "8px 14px",
  borderRadius: 6,
  border: `1px solid ${T.border}`,
  background: T.bg,
  color: T.text,
  fontWeight: 500,
  fontSize: 13,
  cursor: "pointer",
};

const buttonDanger: React.CSSProperties = {
  padding: "8px 14px",
  borderRadius: 6,
  border: "none",
  background: T.error,
  color: "#fff",
  fontWeight: 600,
  fontSize: 13,
  cursor: "pointer",
};

const code: React.CSSProperties = {
  background: T.codeBg,
  padding: "1px 5px",
  borderRadius: 3,
  fontFamily: "monospace",
  fontSize: 11,
};
