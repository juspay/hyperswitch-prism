import { useMemo } from "react";
import { SidebarLayout } from "../components/NavigationSidebar";
import { PrResolverBoard } from "../components/PrResolverBoard";
import { PrResolverSettings } from "../components/PrResolverSettings";
import { usePrResolver, type RuntimeOverlay } from "../hooks/usePrResolver";
import { T } from "../theme";

const CONTROL_WS_PORT =
  (import.meta.env.VITE_WS_PORT as string | undefined) ?? "3142";
const CONTROL_WS_URL = `ws://${location.hostname}:${CONTROL_WS_PORT}`;

/**
 * PR Resolver tab — Kanban view + always-on Settings card. The user
 * configures the resolver here (repo, trigger, interval, etc.) and toggles
 * it on/off without touching config.yml. Saves persist to
 * `~/.byne/pr-resolver-config.json`; the supervisor restarts the service
 * on every successful save.
 */
export function PrResolverPage() {
  const {
    enabled,
    running,
    controlStatus,
    githubRepo,
    trigger,
    lastCycle,
    board,
    processedThreads,
    effectiveConfig,
    runtimeOverlay,
    autoApprove,
    setAutoApprove,
    pollNow,
    updateConfig,
    toggleEnabled,
    lastError,
  } = usePrResolver(CONTROL_WS_URL);

  const totals = useMemo(
    () => ({
      queued: board.queued.length,
      inProgress: board.inProgress.length,
      completed: board.completed.length,
      failed: board.failed.length,
    }),
    [board]
  );
  const totalProcessed = Object.keys(processedThreads).length;

  const handleSave = (overlay: RuntimeOverlay) => {
    updateConfig(overlay);
  };

  const handleReset = () => {
    // Sending an empty overlay clears `~/.byne/pr-resolver-config.json` and
    // falls back to config.yml + env defaults.
    updateConfig({});
  };

  const canEnable = !!effectiveConfig?.githubRepo;

  return (
    <SidebarLayout>
      <style>{`
        @keyframes prResolverPulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.4; }
        }
      `}</style>
      <div
        style={{
          minHeight: "100vh",
          background: T.bg,
          color: T.text,
          fontFamily:
            "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
        }}
      >
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
          <div>
            <h1 style={{ margin: 0, fontSize: 18, fontWeight: 700 }}>
              Byne · PR Resolver
            </h1>
            <span style={{ fontSize: 12, color: T.textMuted }}>
              {enabled ? (
                <>
                  Polling <strong>{githubRepo || "(no repo set)"}</strong> for
                  comments tagged{" "}
                  <code
                    style={{
                      background: T.codeBg,
                      padding: "1px 6px",
                      borderRadius: 3,
                    }}
                  >
                    {trigger || "@trigger"}
                  </code>
                </>
              ) : (
                <>Disabled — toggle on after the settings below are filled in.</>
              )}
            </span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <ConnDot status={controlStatus} />
            <CycleBadge running={running} lastCycle={lastCycle} />
            <AutoApproveToggle
              autoApprove={autoApprove}
              disabled={!enabled || controlStatus !== "open"}
              onToggle={setAutoApprove}
            />
            <EnableToggle
              enabled={enabled}
              disabled={
                controlStatus !== "open" || running || (!enabled && !canEnable)
              }
              title={
                !enabled && !canEnable
                  ? "Set githubRepo first"
                  : running
                    ? "Wait for the cycle to finish"
                    : enabled
                      ? "Disable PR Resolver"
                      : "Enable PR Resolver"
              }
              onToggle={toggleEnabled}
            />
            <button
              onClick={pollNow}
              disabled={!enabled || running || controlStatus !== "open"}
              style={{
                padding: "8px 16px",
                borderRadius: 6,
                border: "none",
                background:
                  !enabled || running || controlStatus !== "open"
                    ? T.border
                    : T.accent,
                color: "#fff",
                fontWeight: 600,
                fontSize: 13,
                cursor:
                  !enabled || running || controlStatus !== "open"
                    ? "not-allowed"
                    : "pointer",
              }}
            >
              {running ? "Polling…" : "Poll Now"}
            </button>
          </div>
        </header>

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

        <PrResolverSettings
          effectiveConfig={effectiveConfig}
          runtimeOverlay={runtimeOverlay}
          running={running}
          onSave={handleSave}
          onReset={handleReset}
        />

        {enabled ? (
          <>
            <SummaryStrip
              queued={totals.queued}
              inProgress={totals.inProgress}
              completed={totals.completed}
              failed={totals.failed}
              processed={totalProcessed}
            />
            <PrResolverBoard board={board} />
          </>
        ) : (
          <DisabledHint canEnable={canEnable} />
        )}
      </div>
    </SidebarLayout>
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
            ? "Auto-approve is ON — every passing PR pushes immediately"
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

function EnableToggle({
  enabled,
  disabled,
  title,
  onToggle,
}: {
  enabled: boolean;
  disabled?: boolean;
  title?: string;
  onToggle: (next: boolean) => void;
}) {
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={() => onToggle(!enabled)}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 8,
        padding: "4px 10px 4px 6px",
        borderRadius: 999,
        border: `1px solid ${enabled ? T.accent : T.border}`,
        background: enabled ? T.accentSoft : T.bg,
        color: enabled ? T.text : T.textMuted,
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
          background: enabled ? T.accent : T.border,
          position: "relative",
          transition: "background 200ms ease",
          display: "inline-block",
        }}
      >
        <span
          style={{
            position: "absolute",
            top: 2,
            left: enabled ? 16 : 2,
            width: 12,
            height: 12,
            borderRadius: "50%",
            background: "#fff",
            transition: "left 200ms ease",
          }}
        />
      </span>
      {enabled ? "Enabled" : "Disabled"}
    </button>
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

function CycleBadge({
  running,
  lastCycle,
}: {
  running: boolean;
  lastCycle: ReturnType<typeof usePrResolver>["lastCycle"];
}) {
  if (running) {
    return (
      <span
        style={{
          fontSize: 12,
          color: T.text,
          background: T.accentSoft,
          padding: "4px 10px",
          borderRadius: 12,
        }}
      >
        Cycle in progress
      </span>
    );
  }
  if (!lastCycle) {
    return (
      <span style={{ fontSize: 12, color: T.textMuted }}>No cycles yet</span>
    );
  }
  return (
    <span
      style={{
        fontSize: 12,
        color: T.textMuted,
        background: T.bgElev,
        padding: "4px 10px",
        borderRadius: 12,
      }}
    >
      Cycle #{lastCycle.cycle} · ✓{lastCycle.fixed} ✕{lastCycle.failed} ↷
      {lastCycle.skipped}
    </span>
  );
}

function SummaryStrip({
  queued,
  inProgress,
  completed,
  failed,
  processed,
}: {
  queued: number;
  inProgress: number;
  completed: number;
  failed: number;
  processed: number;
}) {
  return (
    <div
      style={{
        display: "flex",
        gap: 24,
        padding: "12px 32px",
        background: T.bgRightHeader,
        borderTop: `1px solid ${T.border}`,
        borderBottom: `1px solid ${T.border}`,
        fontSize: 12,
        color: T.textMuted,
      }}
    >
      <Stat label="Queued" value={queued} color="#9ca3af" />
      <Stat label="In Progress" value={inProgress} color="#3b82f6" />
      <Stat label="Completed" value={completed} color={T.success} />
      <Stat label="Failed" value={failed} color={T.error} />
      <span style={{ marginLeft: "auto", color: T.textSubtle }}>
        {processed} thread(s) in history
      </span>
    </div>
  );
}

function Stat({
  label,
  value,
  color,
}: {
  label: string;
  value: number;
  color: string;
}) {
  return (
    <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
      <span
        style={{
          width: 8,
          height: 8,
          borderRadius: "50%",
          background: color,
        }}
      />
      <strong style={{ color: T.text, fontWeight: 600 }}>{value}</strong>
      <span>{label}</span>
    </span>
  );
}

function DisabledHint({ canEnable }: { canEnable: boolean }) {
  return (
    <div
      style={{
        padding: 48,
        textAlign: "center",
        color: T.textMuted,
        fontSize: 14,
      }}
    >
      <div style={{ fontSize: 16, marginBottom: 8, color: T.text }}>
        {canEnable
          ? "Ready when you are."
          : "Set a GitHub repo above to enable."}
      </div>
      <p style={{ lineHeight: 1.6, maxWidth: 540, margin: "0 auto" }}>
        {canEnable ? (
          <>
            Toggle <strong>Enabled</strong> in the header to start polling. The
            supervisor will spin up the service in-process and the board
            below populates as comments come in. Make sure <code style={code}>gh auth login</code>{" "}
            has run once on this host.
          </>
        ) : (
          <>
            Fill in <code style={code}>GitHub repo (owner/name)</code> in the
            Settings card above, save, then flip the toggle on. The resolver
            uses <code style={code}>gh</code> for GitHub auth — run{" "}
            <code style={code}>gh auth login</code> once if you haven't.
          </>
        )}
      </p>
    </div>
  );
}

const code: React.CSSProperties = {
  background: T.codeBg,
  padding: "2px 6px",
  borderRadius: 3,
  fontFamily: "monospace",
  fontSize: 12,
};
