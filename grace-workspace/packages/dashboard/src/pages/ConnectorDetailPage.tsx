import { useState, useMemo, useEffect } from "react";
import { useParams, Link, useNavigate } from "react-router-dom";
import { useSessions } from "../hooks/useSessions";
import { SidebarLayout } from "../components/NavigationSidebar";
import { UnifiedCreateSessionModal, type SessionWithTaskInput } from "../components/UnifiedCreateSessionModal";
import { T } from "../theme";
import type {
  Connector,
  ConnectorPaymentMethod,
  ConnectorFlow,
  ConnectorStatus,
} from "../types/connector";
import connectorsData from "../data/connectors.json";

type ViewAxis = "methods" | "flows";

type Row =
  | { kind: "method"; data: ConnectorPaymentMethod }
  | { kind: "flow"; data: ConnectorFlow };

const CONTROL_WS_PORT =
  (import.meta.env.VITE_WS_PORT as string | undefined) ?? "3142";
const CONTROL_WS_URL = `ws://${location.hostname}:${CONTROL_WS_PORT}`;

type TabType = "supported" | "not-implemented" | "not-supported";

export function ConnectorDetailPage() {
  const { connectorName } = useParams<{ connectorName: string }>();
  const navigate = useNavigate();
  const { sessions, controlStatus, createSession, startSession } = useSessions(CONTROL_WS_URL);
  const [activeTab, setActiveTab] = useState<TabType>("not-implemented");
  const [viewAxis, setViewAxis] = useState<ViewAxis>("methods");
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedMethod, setSelectedMethod] = useState<ConnectorPaymentMethod | null>(null);
  const [selectedFlow, setSelectedFlow] = useState<ConnectorFlow | null>(null);
  const [showTaskModal, setShowTaskModal] = useState(false);
  const [pendingSessionName, setPendingSessionName] = useState<string | null>(null);
  
  const defaultSession = sessions.find((s) => s.sessionId === "default");
  const defaultProjectRoot = defaultSession?.projectRoot ?? "";

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
  
  const connector = useMemo(() => {
    return connectors.find(
      (c) => c.name.toLowerCase() === connectorName?.toLowerCase()
    );
  }, [connectors, connectorName]);

  const tabStatus: Record<TabType, ConnectorStatus> = {
    "supported": "supported",
    "not-implemented": "not_implemented",
    "not-supported": "not_supported",
  };

  const filteredRows = useMemo<Row[]>(() => {
    if (!connector) return [];
    const wanted = tabStatus[activeTab];
    const query = searchQuery.trim().toLowerCase();

    if (viewAxis === "methods") {
      let methods = connector.paymentMethods.filter((m) => m.status === wanted);
      if (query) {
        methods = methods.filter(
          (m) =>
            m.method.toLowerCase().includes(query) ||
            m.category.toLowerCase().includes(query)
        );
      }
      return methods
        .sort((a, b) => a.category.localeCompare(b.category))
        .map<Row>((m) => ({ kind: "method", data: m }));
    }

    let flows = connector.flows.filter((f) => f.status === wanted);
    if (query) {
      flows = flows.filter((f) => f.name.toLowerCase().includes(query));
    }
    return flows
      .sort((a, b) => a.name.localeCompare(b.name))
      .map<Row>((f) => ({ kind: "flow", data: f }));
  }, [connector, activeTab, searchQuery, viewAxis]);

  const stats = useMemo(() => {
    if (!connector) return { supported: 0, notImplemented: 0, notSupported: 0, total: 0 };
    const source = viewAxis === "methods" ? connector.stats : connector.flowStats;
    return {
      supported: source.supported,
      notImplemented: source.notImplemented,
      notSupported: source.notSupported,
      total: source.total,
    };
  }, [connector, viewAxis]);

  const handleRowClick = (row: Row) => {
    if (activeTab !== "not-implemented") return;
    if (row.kind === "method") {
      setSelectedMethod(row.data);
      setShowTaskModal(true);
    } else {
      setSelectedFlow(row.data);
      setShowTaskModal(true);
    }
  };

  if (!connector) {
    return (
      <SidebarLayout>
        <div
          style={{
            minHeight: "100vh",
            background: T.bg,
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            gap: 16,
            color: T.text,
          }}
        >
          <h1 style={{ margin: 0, fontSize: 24 }}>Connector Not Found</h1>
          <p style={{ color: T.textMuted }}>
            Could not find connector &quot;{connectorName}&quot;
          </p>
          <Link
            to="/connectors"
            style={{
              padding: "10px 20px",
              background: T.accent,
              color: "#fff",
              borderRadius: 6,
              textDecoration: "none",
              fontWeight: 600,
            }}
          >
            ← Back to Connectors
          </Link>
        </div>
      </SidebarLayout>
    );
  }

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
        {/* Header */}
        <header
          style={{
            padding: "20px 32px",
            borderBottom: `1px solid ${T.border}`,
            background: T.bgElev,
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 16,
              marginBottom: 16,
            }}
          >
            <Link
              to="/connectors"
              style={{
                padding: "8px 14px",
                borderRadius: 6,
                border: `1px solid ${T.border}`,
                background: T.bg,
                color: T.text,
                textDecoration: "none",
                fontSize: 13,
                fontWeight: 500,
              }}
            >
              ← Back to all connectors
            </Link>
            <ConnDot status={controlStatus} />
          </div>

          <div style={{ display: "flex", alignItems: "baseline", gap: 12 }}>
            <h1 style={{ margin: 0, fontSize: 28, fontWeight: 700 }}>
              {connector.name}
            </h1>
            <a
              href={`/docs/${connector.filePath}`}
              style={{
                fontSize: 13,
                color: T.accent,
                textDecoration: "none",
              }}
            >
              View Documentation →
            </a>
          </div>
        </header>

        {/* Axis pivot — Payment Methods vs Flows */}
        <div
          style={{
            padding: "16px 32px 0",
            background: T.bgSidebar,
            display: "flex",
            gap: 8,
            alignItems: "center",
          }}
        >
          <span style={{ fontSize: 12, color: T.textMuted, fontWeight: 600 }}>
            VIEW:
          </span>
          <AxisPill
            label="Payment Methods"
            active={viewAxis === "methods"}
            onClick={() => setViewAxis("methods")}
          />
          <AxisPill
            label="Flows / APIs"
            active={viewAxis === "flows"}
            onClick={() => setViewAxis("flows")}
          />
        </div>

        {/* Stats Overview */}
        <div
          style={{
            padding: "24px 32px",
            background: T.bgSidebar,
            borderBottom: `1px solid ${T.border}`,
            display: "grid",
            gridTemplateColumns: "repeat(4, 1fr)",
            gap: 20,
            maxWidth: 900,
          }}
        >
          <StatCard
            label="Supported"
            value={stats.supported}
            color={T.success}
            bgColor={T.successSoft}
            isActive={activeTab === "supported"}
            onClick={() => setActiveTab("supported")}
          />
          <StatCard
            label="Not Implemented"
            value={stats.notImplemented}
            color={T.warn}
            bgColor={T.warnSoft}
            isActive={activeTab === "not-implemented"}
            onClick={() => setActiveTab("not-implemented")}
          />
          <StatCard
            label="Not Supported"
            value={stats.notSupported}
            color={T.textSubtle}
            bgColor={T.border}
            isActive={activeTab === "not-supported"}
            onClick={() => setActiveTab("not-supported")}
          />
          <StatCard
            label="Total"
            value={stats.total}
            color={T.text}
            bgColor={T.bgElev}
            isActive={false}
          />
        </div>

        {/* Tab Navigation */}
        <div
          style={{
            padding: "0 32px",
            borderBottom: `1px solid ${T.border}`,
            background: T.bgElev,
            display: "flex",
            gap: 4,
          }}
        >
          <TabButton
            label="Supported"
            count={stats.supported}
            isActive={activeTab === "supported"}
            onClick={() => setActiveTab("supported")}
            color={T.success}
          />
          <TabButton
            label="Not Implemented"
            count={stats.notImplemented}
            isActive={activeTab === "not-implemented"}
            onClick={() => setActiveTab("not-implemented")}
            color={T.warn}
          />
          <TabButton
            label="Not Supported"
            count={stats.notSupported}
            isActive={activeTab === "not-supported"}
            onClick={() => setActiveTab("not-supported")}
            color={T.textSubtle}
          />
        </div>

        {/* Search and Table */}
        <section style={{ padding: "24px 32px" }}>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              marginBottom: 20,
            }}
          >
            <h2 style={{ margin: 0, fontSize: 18, fontWeight: 600 }}>
              {(() => {
                const noun = viewAxis === "methods" ? "Payment Methods" : "Flows";
                if (activeTab === "supported") return `Supported ${noun}`;
                if (activeTab === "not-implemented") return `Not Implemented ${noun}`;
                return `Not Supported ${noun}`;
              })()}
            </h2>
            <div style={{ position: "relative" }}>
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder={
                  viewAxis === "methods"
                    ? "Search payment methods..."
                    : "Search flows..."
                }
                style={{
                  padding: "8px 12px 8px 36px",
                  borderRadius: 6,
                  border: `1px solid ${T.border}`,
                  background: T.bgElev,
                  color: T.text,
                  fontSize: 13,
                  width: 280,
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
          </div>

          {/* Payment Methods Table */}
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
                fontSize: 14,
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
                      width: "40%",
                    }}
                  >
                    {viewAxis === "methods" ? "Payment Method" : "Flow"}
                  </th>
                  <th
                    style={{
                      padding: "14px 20px",
                      textAlign: "left",
                      fontWeight: 600,
                      color: T.text,
                      borderBottom: `1px solid ${T.border}`,
                      width: "30%",
                    }}
                  >
                    {viewAxis === "methods" ? "Category" : "Service"}
                  </th>
                  <th
                    style={{
                      padding: "14px 20px",
                      textAlign: "center",
                      fontWeight: 600,
                      color: T.text,
                      borderBottom: `1px solid ${T.border}`,
                      width: "30%",
                    }}
                  >
                    Status
                  </th>
                </tr>
              </thead>
              <tbody>
                {filteredRows.length === 0 ? (
                  <tr>
                    <td
                      colSpan={3}
                      style={{
                        padding: "40px 20px",
                        textAlign: "center",
                        color: T.textMuted,
                      }}
                    >
                      No {viewAxis === "methods" ? "payment methods" : "flows"} found in this category.
                      {searchQuery && " Try adjusting your search."}
                    </td>
                  </tr>
                ) : (
                  filteredRows.map((row, idx) => (
                    <DataRow
                      key={
                        row.kind === "method"
                          ? `m-${row.data.category}-${row.data.method}`
                          : `f-${row.data.name}`
                      }
                      row={row}
                      isEven={idx % 2 === 1}
                      isClickable={activeTab === "not-implemented"}
                      onClick={() => handleRowClick(row)}
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
            }}
          >
            Showing {filteredRows.length}{" "}
            {activeTab === "supported" && "supported"}
            {activeTab === "not-implemented" && "not implemented"}
            {activeTab === "not-supported" && "not supported"}{" "}
            {viewAxis === "methods" ? "payment methods" : "flows"}
          </div>
        </section>

        {/* Task Modal — payment method gap */}
        {showTaskModal && selectedMethod && connector && (
          <UnifiedCreateSessionModal
            defaultSourcePath={defaultProjectRoot}
            defaultTaskValues={{
              title: `Implement ${selectedMethod.method} for ${connector.name}`,
              paymentMethod: selectedMethod.method,
              category: selectedMethod.category,
              targetConnectors: [connector.name],
              description: `Implement ${selectedMethod.method} payment method (${selectedMethod.category}) for ${connector.name} connector.\n\nDocumentation: ${connector.filePath}`,
            }}
            onCreate={(input: SessionWithTaskInput) => {
              createSession(input);
              setShowTaskModal(false);
              setSelectedMethod(null);
            }}
            onCreateAndStart={async (input: SessionWithTaskInput) => {
              setPendingSessionName(input.name);
              createSession(input);
              setShowTaskModal(false);
              setSelectedMethod(null);
            }}
            onClose={() => {
              setShowTaskModal(false);
              setSelectedMethod(null);
            }}
            wsConnected={controlStatus === "open"}
          />
        )}

        {/* Task Modal — flow gap */}
        {showTaskModal && selectedFlow && connector && (
          <UnifiedCreateSessionModal
            defaultSourcePath={defaultProjectRoot}
            defaultTaskValues={{
              title: `Implement ${selectedFlow.name} flow for ${connector.name}`,
              flow: selectedFlow.name,
              targetConnectors: [connector.name],
              description:
                `Implement the ${selectedFlow.name} flow for ${connector.name} connector.\n\n` +
                `Documentation: ${connector.filePath}`,
            }}
            onCreate={(input: SessionWithTaskInput) => {
              createSession(input);
              setShowTaskModal(false);
              setSelectedFlow(null);
            }}
            onCreateAndStart={async (input: SessionWithTaskInput) => {
              setPendingSessionName(input.name);
              createSession(input);
              setShowTaskModal(false);
              setSelectedFlow(null);
            }}
            onClose={() => {
              setShowTaskModal(false);
              setSelectedFlow(null);
            }}
            wsConnected={controlStatus === "open"}
          />
        )}
      </div>
    </SidebarLayout>
  );
}

function StatCard({
  label,
  value,
  color,
  bgColor,
  isActive,
  onClick,
}: {
  label: string;
  value: number;
  color: string;
  bgColor: string;
  isActive: boolean;
  onClick?: () => void;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        padding: "20px",
        borderRadius: 10,
        background: isActive ? bgColor : T.bgElev,
        border: `2px solid ${isActive ? color : T.border}`,
        cursor: onClick !== undefined ? "pointer" : "default",
        textAlign: "center",
        transition: "all 150ms",
      }}
    >
      <div style={{ fontSize: 32, fontWeight: 700, color }}>{value}</div>
      <div
        style={{
          fontSize: 12,
          fontWeight: 600,
          color: isActive ? color : T.textMuted,
          textTransform: "uppercase",
          letterSpacing: 0.5,
          marginTop: 4,
        }}
      >
        {label}
      </div>
    </button>
  );
}

function TabButton({
  label,
  count,
  isActive,
  onClick,
  color,
}: {
  label: string;
  count: number;
  isActive: boolean;
  onClick: () => void;
  color: string;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        padding: "14px 20px",
        border: "none",
        borderBottom: `3px solid ${isActive ? color : "transparent"}`,
        background: "transparent",
        color: isActive ? color : T.textMuted,
        fontWeight: isActive ? 700 : 500,
        fontSize: 14,
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        gap: 8,
        transition: "all 150ms",
      }}
    >
      <span>{label}</span>
      <span
        style={{
          padding: "2px 8px",
          borderRadius: 999,
          background: isActive ? color : T.border,
          color: isActive ? "#fff" : T.textSubtle,
          fontSize: 12,
          fontWeight: 700,
        }}
      >
        {count}
      </span>
    </button>
  );
}

const STATUS_CONFIG: Record<
  ConnectorStatus,
  { label: string; color: string; bgColor: string }
> = {
  supported: { label: "✓ Supported", color: T.success, bgColor: T.successSoft },
  not_implemented: { label: "⚠ Not Implemented", color: T.warn, bgColor: T.warnSoft },
  not_supported: { label: "✕ Not Supported", color: T.textSubtle, bgColor: T.border },
  error: { label: "? Error", color: T.error, bgColor: T.errorSoft },
};

function flowService(flowName: string): string {
  // "Pay.Capture" → "Pay"; "MerchantAuthentication.Authenticate" → "MerchantAuthentication".
  // Plain names (e.g. "PaymentService.Authorize") split on the first ".".
  const dot = flowName.indexOf(".");
  return dot > 0 ? flowName.slice(0, dot) : "—";
}

function flowLabel(flowName: string): string {
  const dot = flowName.indexOf(".");
  return dot > 0 ? flowName.slice(dot + 1) : flowName;
}

function DataRow({
  row,
  isEven,
  isClickable,
  onClick,
}: {
  row: Row;
  isEven: boolean;
  isClickable: boolean;
  onClick: () => void;
}) {
  const status = STATUS_CONFIG[row.kind === "method" ? row.data.status : row.data.status];
  const primaryLabel =
    row.kind === "method" ? row.data.method : flowLabel(row.data.name);
  const secondaryLabel =
    row.kind === "method" ? row.data.category : flowService(row.data.name);

  return (
    <tr
      onClick={isClickable ? onClick : undefined}
      style={{
        background: isEven ? T.bg : T.bgElev,
        cursor: isClickable ? "pointer" : "default",
        transition: "background 150ms",
      }}
      onMouseEnter={(e) => {
        if (isClickable) {
          e.currentTarget.style.background = T.accentSoft;
        }
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = isEven ? T.bg : T.bgElev;
      }}
    >
      <td
        style={{
          padding: "14px 20px",
          borderBottom: `1px solid ${T.border}`,
          fontWeight: 500,
        }}
      >
        {primaryLabel}
        {isClickable && (
          <span
            style={{
              marginLeft: 8,
              fontSize: 11,
              color: T.accent,
              fontWeight: 600,
            }}
          >
            (Click to create task)
          </span>
        )}
      </td>
      <td
        style={{
          padding: "14px 20px",
          borderBottom: `1px solid ${T.border}`,
          color: T.textMuted,
        }}
      >
        {secondaryLabel}
      </td>
      <td
        style={{
          padding: "14px 20px",
          borderBottom: `1px solid ${T.border}`,
          textAlign: "center",
        }}
      >
        <span
          style={{
            padding: "6px 12px",
            borderRadius: 999,
            background: status.bgColor,
            color: status.color,
            fontWeight: 600,
            fontSize: 12,
          }}
        >
          {status.label}
        </span>
      </td>
    </tr>
  );
}

function AxisPill({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        padding: "6px 14px",
        borderRadius: 999,
        border: `1px solid ${active ? T.accent : T.border}`,
        background: active ? T.accentSoft : "transparent",
        color: active ? T.accent : T.textMuted,
        fontSize: 12,
        fontWeight: 600,
        cursor: "pointer",
        transition: "all 150ms",
      }}
    >
      {label}
    </button>
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
