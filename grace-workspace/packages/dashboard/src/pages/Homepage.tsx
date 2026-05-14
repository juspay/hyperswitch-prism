import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useSessions, type SessionRecord } from "../hooks/useSessions";
import { UnifiedCreateSessionModal } from "../components/UnifiedCreateSessionModal";
import { SidebarLayout } from "../components/NavigationSidebar";
import { T } from "../theme";
import type { SessionWithTaskInput } from "../components/UnifiedCreateSessionModal";

const CONTROL_WS_PORT =
  (import.meta.env.VITE_WS_PORT as string | undefined) ?? "3142";
const CONTROL_WS_URL = `ws://${location.hostname}:${CONTROL_WS_PORT}`;

/**
 * Sessions index. Each card is a portal into one isolated workspace.
 * Clicking a card navigates to /sessions/<id> where WorkflowPage takes
 * over. Creating a session calls the supervisor and routes to the new
 * session as soon as it appears in the list.
 */
export function Homepage() {
  const {
    sessions,
    controlStatus,
    lastError,
    createSession,
    startSession,
  } = useSessions(CONTROL_WS_URL);
  const navigate = useNavigate();
  const [showCreate, setShowCreate] = useState(false);
  const [pendingCreateName, setPendingCreateName] = useState<string | null>(null);
  const [shouldAutoStart, setShouldAutoStart] = useState(false);

  // Auto-route to a new session as soon as it appears. Done in an effect
  // (not inline during render) so React doesn't swallow the navigate() call
  // when StrictMode replays the render.
  const newlyCreated = useMemo(() => {
    if (!pendingCreateName) return undefined;
    return sessions.find((s) => s.name === pendingCreateName);
  }, [pendingCreateName, sessions]);
  useEffect(() => {
    if (newlyCreated) {
      const sessionId = newlyCreated.sessionId;
      setPendingCreateName(null);
      
      if (shouldAutoStart) {
        setShouldAutoStart(false);
        // Start the engine immediately, then navigate
        startSession(sessionId).then(() => {
          navigate(`/sessions/${sessionId}`);
        });
      } else {
        // Just navigate (existing behavior)
        navigate(`/sessions/${sessionId}`, {
          state: { autostart: true },
        });
      }
    }
  }, [newlyCreated, navigate, shouldAutoStart, startSession]);

  const defaultSession = sessions.find((s) => s.sessionId === "default");
  const defaultProjectRoot = defaultSession?.projectRoot ?? "";

  const active = sessions.filter((s) => s.status !== "archived");
  const archived = sessions.filter((s) => s.status === "archived");

  return (
    <SidebarLayout>
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
        }}
      >
        <div>
          <h1 style={{ margin: 0, fontSize: 18, fontWeight: 700 }}>10XGRACE · Sessions</h1>
          <span style={{ fontSize: 12, color: T.textMuted }}>
            Each session is an isolated worktree on disk + its own engine process.
          </span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <ConnDot status={controlStatus} />
          <button
            onClick={() => setShowCreate(true)}
            disabled={controlStatus !== "open"}
            style={{
              padding: "8px 16px",
              borderRadius: 6,
              border: "none",
              background: controlStatus === "open" ? T.accent : T.border,
              color: "#fff",
              fontWeight: 600,
              fontSize: 13,
              cursor: controlStatus === "open" ? "pointer" : "not-allowed",
            }}
          >
            + New Session
          </button>
        </div>
      </header>

      {lastError && (
        <div
          style={{
            margin: "16px 32px 0",
            padding: "10px 14px",
            borderRadius: 6,
            background: T.errorSoft,
            color: T.error,
            fontSize: 12,
            border: `1px solid ${T.error}`,
          }}
        >
          {lastError.kind}: {lastError.message}
        </div>
      )}

      <section style={{ padding: "24px 32px" }}>
        <h2 style={sectionTitleStyle}>Active</h2>
        <div style={gridStyle}>
          {active.map((s) => (
            <SessionCard
              key={s.sessionId}
              session={s}
              onClick={() => navigate(`/sessions/${s.sessionId}`)}
            />
          ))}
          <CreateTile onClick={() => setShowCreate(true)} disabled={controlStatus !== "open"} />
        </div>
      </section>

      {archived.length > 0 && (
        <section style={{ padding: "0 32px 24px" }}>
          <h2 style={sectionTitleStyle}>Archived</h2>
          <div style={gridStyle}>
            {archived.map((s) => (
              <SessionCard
                key={s.sessionId}
                session={s}
                onClick={() => navigate(`/sessions/${s.sessionId}`)}
              />
            ))}
          </div>
        </section>
      )}

        {showCreate && (
          <UnifiedCreateSessionModal
            defaultSourcePath={defaultProjectRoot}
            onCreate={(input: SessionWithTaskInput) => {
              setPendingCreateName(input.name);
              createSession(input);
              setShowCreate(false);
            }}
            onCreateAndStart={async (input: SessionWithTaskInput) => {
              setPendingCreateName(input.name);
              createSession(input);
              setShowCreate(false);
              
              // Wait for session to appear in list, then start it
              const checkAndStart = setInterval(async () => {
                const newSession = sessions.find((s) => s.name === input.name && s.status === "idle");
                if (newSession) {
                  clearInterval(checkAndStart);
                  await startSession(newSession.sessionId);
                  navigate(`/sessions/${newSession.sessionId}`, {
                    state: { autostart: true },
                  });
                }
              }, 500);
              
              // Cleanup after 10 seconds if session never appears
              setTimeout(() => clearInterval(checkAndStart), 10000);
            }}
            onClose={() => setShowCreate(false)}
            wsConnected={controlStatus === "open"}
          />
        )}
      </div>
    </SidebarLayout>
  );
}

function SessionCard({
  session,
  onClick,
}: {
  session: SessionRecord;
  onClick: () => void;
}) {
  const archived = session.status === "archived";
  const running = session.status === "running";
  const dot = archived ? T.textSubtle : running ? T.success : T.accent;
  return (
    <div
      onClick={onClick}
      style={{
        background: T.bgElev,
        border: `1px solid ${T.border}`,
        borderRadius: 10,
        padding: 16,
        cursor: "pointer",
        boxShadow: T.shadow,
        display: "flex",
        flexDirection: "column",
        gap: 8,
        opacity: archived ? 0.7 : 1,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span
          style={{
            width: 8,
            height: 8,
            borderRadius: "50%",
            background: dot,
            animation: running ? "pulse 1.6s ease-in-out infinite" : undefined,
          }}
        />
        <span style={{ fontSize: 11, fontWeight: 600, color: T.textSubtle, textTransform: "uppercase", letterSpacing: 0.5 }}>
          {archived ? "Archived" : running ? "Running" : "Idle"}
        </span>
      </div>
      <h3 style={{ margin: 0, fontSize: 15, fontWeight: 700, color: T.text }}>
        {session.name}
      </h3>
      {session.description && (
        <p style={{ margin: 0, fontSize: 12, color: T.textMuted, lineHeight: 1.4 }}>
          {session.description}
        </p>
      )}
      <div style={{ fontSize: 11, color: T.textSubtle, fontFamily: "ui-monospace, monospace", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        {session.projectRoot}
      </div>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", fontSize: 11, color: T.textMuted, marginTop: 4 }}>
        <span>{relTime(session.updatedAt)}</span>
        {session.wsPort != null && <span>ws {session.wsPort}</span>}
      </div>
    </div>
  );
}

function CreateTile({ onClick, disabled }: { onClick: () => void; disabled: boolean }) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      style={{
        background: "transparent",
        border: `2px dashed ${T.border}`,
        borderRadius: 10,
        padding: 16,
        cursor: disabled ? "not-allowed" : "pointer",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 6,
        color: T.textMuted,
        minHeight: 130,
      }}
    >
      <span style={{ fontSize: 28, lineHeight: 1 }}>+</span>
      <span style={{ fontSize: 13, fontWeight: 600 }}>Create New Session</span>
    </button>
  );
}

function ConnDot({ status }: { status: "connecting" | "open" | "closed" }) {
  const color =
    status === "open" ? T.success : status === "connecting" ? T.warn : T.error;
  const label =
    status === "open" ? "supervisor connected" : status === "connecting" ? "connecting…" : "disconnected";
  return (
    <span style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 11, color: T.textMuted }}>
      <span style={{ width: 7, height: 7, borderRadius: "50%", background: color }} />
      {label}
    </span>
  );
}

function relTime(ts: number): string {
  const diff = Date.now() - ts;
  const min = Math.floor(diff / 60_000);
  if (min < 1) return "just now";
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const d = Math.floor(hr / 24);
  return `${d}d ago`;
}

const sectionTitleStyle: React.CSSProperties = {
  margin: "0 0 12px 0",
  fontSize: 12,
  fontWeight: 700,
  textTransform: "uppercase",
  letterSpacing: 1,
  color: T.textMuted,
};

const gridStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(auto-fill, minmax(260px, 1fr))",
  gap: 14,
};
