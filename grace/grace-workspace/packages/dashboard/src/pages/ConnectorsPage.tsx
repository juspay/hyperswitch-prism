import { useState, useMemo, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { useSessions } from "../hooks/useSessions";
import { SidebarLayout } from "../components/NavigationSidebar";
import { UnifiedCreateSessionModal, type SessionWithTaskInput } from "../components/UnifiedCreateSessionModal";
import { T } from "../theme";
import type { Connector, MethodGap } from "../types/connector";
import connectorsData from "../data/connectors.json";
import { Link } from "react-router-dom";

const CONTROL_WS_PORT =
  (import.meta.env.VITE_WS_PORT as string | undefined) ?? "3142";
const CONTROL_WS_URL = `ws://${location.hostname}:${CONTROL_WS_PORT}`;

function generateDefaultDescription(connector: Connector, notImplemented: MethodGap[]): string {
  const lines = [
    `Implement payment flows for ${connector.name} connector.`,
    "",
    "Current Status:",
    `- Supported: ${connector.stats.supported} payment methods`,
    `- Not Implemented: ${connector.stats.notImplemented} payment methods`,
    `- Not Supported: ${connector.stats.notSupported} payment methods`,
    "",
    "Target Implementation:",
  ];

  if (notImplemented.length > 0) {
    const byCategory = new Map<string, string[]>();
    for (const item of notImplemented) {
      const existing = byCategory.get(item.category) || [];
      existing.push(item.method);
      byCategory.set(item.category, existing);
    }

    for (const [category, methods] of byCategory) {
      lines.push(`- ${category}: ${methods.join(", ")}`);
    }
  }

  lines.push("", `Documentation: ${connector.filePath}`);

  return lines.join("\n");
}

export function ConnectorsPage() {
  const { sessions, controlStatus, lastError, createSession, startSession } = useSessions(CONTROL_WS_URL);
  const navigate = useNavigate();
  
  const defaultSession = sessions.find((s) => s.sessionId === "default");
  const defaultProjectRoot = defaultSession?.projectRoot ?? "";
  const [searchQuery, setSearchQuery] = useState("");
  const [showOnlyNotImplemented, setShowOnlyNotImplemented] = useState(false);
  const [selectedConnector, setSelectedConnector] = useState<Connector | null>(null);
  const [selectedNotImplemented, setSelectedNotImplemented] = useState<MethodGap[]>([]);
  const [showTaskModal, setShowTaskModal] = useState(false);
  const [pendingSessionName, setPendingSessionName] = useState<string | null>(null);
  
  // Watch for newly created session and auto-start if needed
  useEffect(() => {
    if (pendingSessionName) {
      const newSession = sessions.find((s) => s.name === pendingSessionName && s.status === "idle");
      if (newSession) {
        setPendingSessionName(null);
        startSession(newSession.sessionId).then(() => {
          navigate(`/sessions/${newSession.sessionId}`);
        });
      }
    }
  }, [sessions, pendingSessionName, startSession, navigate]);

  const connectors = useMemo(() => connectorsData as Connector[], []);

  const filteredConnectors = useMemo(() => {
    let filtered = connectors;

    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter(
        (c) =>
          c.name.toLowerCase().includes(query) ||
          c.paymentMethods.some(
            (pm) =>
              pm.category.toLowerCase().includes(query) ||
              pm.method.toLowerCase().includes(query)
          )
      );
    }

    if (showOnlyNotImplemented) {
      filtered = filtered.filter(
        (c) => c.stats.notImplemented > 0 || c.flowStats.notImplemented > 0
      );
    }

    return filtered.sort((a, b) => a.name.localeCompare(b.name));
  }, [connectors, searchQuery, showOnlyNotImplemented]);

  const totalNotImplemented = useMemo(
    () => connectors.reduce((sum, c) => sum + c.stats.notImplemented, 0),
    [connectors]
  );

  const handleRowClick = (connector: Connector) => {
    // Find all not-implemented items for this connector
    const notImplemented: MethodGap[] = connector.paymentMethods
      .filter((pm) => pm.status === "not_implemented")
      .map((pm) => ({
        kind: "method",
        connector: connector.name,
        category: pm.category,
        method: pm.method,
        filePath: connector.filePath,
      }));

    setSelectedConnector(connector);
    setSelectedNotImplemented(notImplemented);
    setShowTaskModal(true);
  };

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
        <style>{`
          html, body, #root { margin: 0; padding: 0; background: ${T.bg}; }
          * { box-sizing: border-box; }
        `}</style>

        {/* Header */}
        <header
          style={{
            padding: "20px 32px",
            borderBottom: `1px solid ${T.border}`,
            background: T.bgElev,
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            flexWrap: "wrap",
            gap: 16,
          }}
        >
          <div>
            <h1 style={{ margin: 0, fontSize: 18, fontWeight: 700 }}>
              Payment Processors
            </h1>
            <span style={{ fontSize: 12, color: T.textMuted }}>
              {connectors.length} connectors · {totalNotImplemented} not implemented
            </span>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <ConnDot status={controlStatus} />

            {/* Search */}
            <div style={{ position: "relative" }}>
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search connectors..."
                style={{
                  padding: "8px 12px 8px 36px",
                  borderRadius: 6,
                  border: `1px solid ${T.border}`,
                  background: T.bg,
                  color: T.text,
                  fontSize: 13,
                  width: 240,
                  outline: "none",
                }}
              />
              <span
                style={{
                  position: "absolute",
                  left: 12,
                  top: "50%",
                  transform: "translateY(-50%)",
                  color: T.textSubtle,
                  fontSize: 14,
                }}
              >
                🔍
              </span>
            </div>

            {/* Filter Toggle */}
            <label
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                padding: "8px 14px",
                background: showOnlyNotImplemented ? T.accentSoft : T.bg,
                border: `1px solid ${showOnlyNotImplemented ? T.accent : T.border}`,
                borderRadius: 6,
                cursor: "pointer",
                fontSize: 13,
                color: showOnlyNotImplemented ? T.accent : T.text,
                fontWeight: showOnlyNotImplemented ? 600 : 400,
              }}
            >
              <input
                type="checkbox"
                checked={showOnlyNotImplemented}
                onChange={(e) => setShowOnlyNotImplemented(e.target.checked)}
                style={{ display: "none" }}
              />
              <span>⚠️ Show only with gaps</span>
            </label>
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

        {/* Stats Bar */}
        <div
          style={{
            padding: "16px 32px",
            background: T.bgSidebar,
            borderBottom: `1px solid ${T.border}`,
            display: "flex",
            gap: 24,
            fontSize: 13,
          }}
        >
          <StatBadge label="Total" value={connectors.length} color={T.text} />
          <StatBadge
            label="With Not Implemented"
            value={connectors.filter((c) => c.stats.notImplemented > 0).length}
            color={T.warn}
            bgColor={T.warnSoft}
          />
          <StatBadge
            label="Complete"
            value={connectors.filter((c) => c.stats.notImplemented === 0).length}
            color={T.success}
            bgColor={T.successSoft}
          />
          <StatBadge
            label="Flow Gaps"
            value={connectors.reduce((s, c) => s + c.flowStats.notImplemented, 0)}
            color={T.warn}
            bgColor={T.warnSoft}
          />
        </div>

        {/* Table */}
        <section style={{ padding: "24px 32px" }}>
          <div
            style={{
              background: T.bgElev,
              border: `1px solid ${T.border}`,
              borderRadius: 10,
              overflow: "hidden",
              boxShadow: T.shadow,
            }}
          >
            <table
              style={{
                width: "100%",
                borderCollapse: "collapse",
                fontSize: 13,
              }}
            >
              <thead>
                <tr style={{ background: T.bgSidebar }}>
                  <th
                    style={{
                      padding: "14px 20px",
                      textAlign: "left",
                      fontWeight: 600,
                      color: T.text,
                      borderBottom: `1px solid ${T.border}`,
                    }}
                  >
                    Connector
                  </th>
                  <th
                    style={{
                      padding: "14px 16px",
                      textAlign: "center",
                      fontWeight: 600,
                      color: T.text,
                      borderBottom: `1px solid ${T.border}`,
                    }}
                  >
                    Supported
                  </th>
                  <th
                    style={{
                      padding: "14px 16px",
                      textAlign: "center",
                      fontWeight: 600,
                      color: T.text,
                      borderBottom: `1px solid ${T.border}`,
                    }}
                  >
                    Not Implemented
                  </th>
                  <th
                    style={{
                      padding: "14px 16px",
                      textAlign: "center",
                      fontWeight: 600,
                      color: T.text,
                      borderBottom: `1px solid ${T.border}`,
                    }}
                  >
                    Not Supported
                  </th>
                  <th
                    style={{
                      padding: "14px 16px",
                      textAlign: "center",
                      fontWeight: 600,
                      color: T.text,
                      borderBottom: `1px solid ${T.border}`,
                    }}
                  >
                    Flows
                  </th>
                  <th
                    style={{
                      padding: "14px 16px",
                      textAlign: "center",
                      fontWeight: 600,
                      color: T.text,
                      borderBottom: `1px solid ${T.border}`,
                    }}
                  >
                    Status
                  </th>
                </tr>
              </thead>
              <tbody>
                {filteredConnectors.length === 0 ? (
                  <tr>
                    <td
                      colSpan={6}
                      style={{
                        padding: "40px 20px",
                        textAlign: "center",
                        color: T.textMuted,
                      }}
                    >
                      No connectors found matching your search.
                    </td>
                  </tr>
                ) : (
                  filteredConnectors.map((connector, idx) => (
                    <ConnectorRow
                      key={connector.name}
                      connector={connector}
                      isEven={idx % 2 === 1}
                    />
                  ))
                )}
              </tbody>
            </table>
          </div>

          <div
            style={{
              marginTop: 16,
              fontSize: 12,
              color: T.textSubtle,
              textAlign: "center",
            }}
          >
            Showing {filteredConnectors.length} of {connectors.length} connectors
            {showOnlyNotImplemented && " (filtered to not-implemented only)"}
          </div>
        </section>

        {/* Task Modal */}
        {showTaskModal && selectedConnector && (
          <UnifiedCreateSessionModal
            defaultSourcePath={defaultProjectRoot}
            defaultTaskValues={{
              title: `Implement ${selectedConnector.name} Payment Flows`,
              targetConnectors: [selectedConnector.name],
              description: generateDefaultDescription(selectedConnector, selectedNotImplemented),
            }}
            onCreate={(input: SessionWithTaskInput) => {
              createSession(input);
              setShowTaskModal(false);
              setSelectedConnector(null);
              setSelectedNotImplemented([]);
            }}
            onCreateAndStart={async (input: SessionWithTaskInput) => {
              // Create session with task metadata
              createSession(input);
              setShowTaskModal(false);
              // Note: We'll need to add auto-start logic similar to Homepage
              // For now, session will be created and appear in the list
              setSelectedConnector(null);
              setSelectedNotImplemented([]);
            }}
            onClose={() => {
              setShowTaskModal(false);
              setSelectedConnector(null);
              setSelectedNotImplemented([]);
            }}
            wsConnected={controlStatus === "open"}
          />
        )}
      </div>
    </SidebarLayout>
  );
}

function StatBadge({
  label,
  value,
  color,
  bgColor,
}: {
  label: string;
  value: number;
  color: string;
  bgColor?: string;
}) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <span style={{ color: T.textMuted }}>{label}:</span>
      <span
        style={{
          padding: "4px 10px",
          borderRadius: 999,
          background: bgColor || T.border,
          color: color,
          fontWeight: 700,
          fontSize: 13,
        }}
      >
        {value}
      </span>
    </div>
  );
}

function ConnectorRow({
  connector,
  isEven,
}: {
  connector: Connector;
  isEven: boolean;
}) {
  const hasNotImplemented = connector.stats.notImplemented > 0;
  const completionRate =
    connector.stats.total > 0
      ? Math.round(
          (connector.stats.supported / connector.stats.total) * 100
        )
      : 0;

  let statusColor = T.success;
  let statusLabel = "Complete";
  if (hasNotImplemented) {
    statusColor = T.warn;
    statusLabel = `${connector.stats.notImplemented} pending`;
  }
  if (connector.stats.notSupported > connector.stats.supported) {
    statusColor = T.textSubtle;
    statusLabel = "Limited";
  }

  return (
    <tr
      style={{
        background: isEven ? T.bg : T.bgElev,
        transition: "background 150ms",
      }}
    >
      <td
        style={{
          padding: "14px 20px",
          borderBottom: `1px solid ${T.border}`,
        }}
      >
        <Link
          to={`/connectors/${connector.name}`}
          style={{
            fontWeight: 600,
            color: T.text,
            textDecoration: "none",
            fontSize: 15,
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.color = T.accent;
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.color = T.text;
          }}
        >
          {connector.name}
        </Link>
        <div style={{ fontSize: 11, color: T.textSubtle, marginTop: 2 }}>
          <a
            href={`/docs/${connector.filePath}`}
            onClick={(e) => e.stopPropagation()}
            style={{
              color: T.accent,
              textDecoration: "none",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.textDecoration = "underline";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.textDecoration = "none";
            }}
          >
            {connector.filePath}
          </a>
        </div>
        <div style={{ fontSize: 11, color: T.textMuted, marginTop: 4 }}>
          Completion: {completionRate}%
        </div>
      </td>
      <td
        style={{
          padding: "14px 16px",
          textAlign: "center",
          borderBottom: `1px solid ${T.border}`,
        }}
      >
        <CountBadge
          count={connector.stats.supported}
          color={T.success}
          bgColor={T.successSoft}
        />
      </td>
      <td
        style={{
          padding: "14px 16px",
          textAlign: "center",
          borderBottom: `1px solid ${T.border}`,
        }}
      >
        <CountBadge
          count={connector.stats.notImplemented}
          color={hasNotImplemented ? T.warn : T.textSubtle}
          bgColor={hasNotImplemented ? T.warnSoft : T.border}
        />
      </td>
      <td
        style={{
          padding: "14px 16px",
          textAlign: "center",
          borderBottom: `1px solid ${T.border}`,
        }}
      >
        <CountBadge
          count={connector.stats.notSupported}
          color={T.textSubtle}
          bgColor={T.border}
        />
      </td>
      <td
        style={{
          padding: "14px 16px",
          textAlign: "center",
          borderBottom: `1px solid ${T.border}`,
        }}
      >
        <FlowCluster stats={connector.flowStats} />
      </td>
      <td
        style={{
          padding: "14px 16px",
          textAlign: "center",
          borderBottom: `1px solid ${T.border}`,
        }}
      >
        <span
          style={{
            padding: "4px 10px",
            borderRadius: 999,
            background: statusColor + "20",
            color: statusColor,
            fontWeight: 600,
            fontSize: 12,
          }}
        >
          {statusLabel}
        </span>
      </td>
    </tr>
  );
}

function FlowCluster({ stats }: { stats: Connector["flowStats"] }) {
  return (
    <Link
      to="#"
      onClick={(e) => e.preventDefault()}
      style={{
        display: "inline-flex",
        gap: 6,
        alignItems: "center",
        fontSize: 11,
        fontWeight: 600,
        color: T.textMuted,
        cursor: "default",
        textDecoration: "none",
      }}
      title={`${stats.supported} supported · ${stats.notImplemented} not implemented · ${stats.notSupported} not supported (of ${stats.total} flows)`}
    >
      <span style={{ color: T.success }}>{stats.supported}✓</span>
      <span style={{ color: stats.notImplemented > 0 ? T.warn : T.textSubtle }}>
        {stats.notImplemented}⚠
      </span>
      <span style={{ color: T.textSubtle }}>{stats.notSupported}✕</span>
    </Link>
  );
}

function CountBadge({
  count,
  color,
  bgColor,
}: {
  count: number;
  color: string;
  bgColor: string;
}) {
  return (
    <span
      style={{
        display: "inline-block",
        minWidth: 28,
        padding: "4px 8px",
        borderRadius: 999,
        background: bgColor,
        color: color,
        fontWeight: 600,
        fontSize: 12,
      }}
    >
      {count}
    </span>
  );
}

function ConnDot({ status }: { status: "connecting" | "open" | "closed" }) {
  const color =
    status === "open" ? T.success : status === "connecting" ? T.warn : T.error;
  const label =
    status === "open"
      ? "connected"
      : status === "connecting"
      ? "connecting…"
      : "disconnected";
  return (
    <span
      style={{
        display: "flex",
        alignItems: "center",
        gap: 6,
        fontSize: 11,
        color: T.textMuted,
      }}
    >
      <span
        style={{
          width: 7,
          height: 7,
          borderRadius: "50%",
          background: color,
        }}
      />
      {label}
    </span>
  );
}
