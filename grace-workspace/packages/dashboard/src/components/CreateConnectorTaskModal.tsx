import { useState, useMemo } from "react";
import { T } from "../theme";
import type { Connector, MethodGap, ConnectorPaymentMethod } from "../types/connector";
import type { SessionCopyStrategy } from "../hooks/useSessions";

interface Props {
  connector: Connector;
  notImplemented?: MethodGap[];
  selectedMethod?: ConnectorPaymentMethod;
  onClose: () => void;
  onCreateSession: (input: {
    name: string;
    description?: string;
    sourcePath: string;
    strategy: SessionCopyStrategy;
  }) => void;
  wsConnected: boolean;
}

/**
 * Modal for creating a new session with pre-filled connector task details.
 * Shows the connector info and allows customizing the task before creating.
 * 
 * Usage:
 * - From ConnectorsPage: Pass notImplemented array
 * - From ConnectorDetailPage: Pass selectedMethod for single payment method
 */
export function CreateConnectorTaskModal({
  connector,
  notImplemented,
  selectedMethod,
  onClose,
  onCreateSession,
  wsConnected,
}: Props) {
  // Determine mode: single method or multiple methods
  const isSingleMethodMode = !!selectedMethod;
  
  const items = useMemo<MethodGap[]>(() => {
    if (selectedMethod) {
      return [{
        kind: "method",
        connector: connector.name,
        category: selectedMethod.category,
        method: selectedMethod.method,
        filePath: connector.filePath,
      }];
    }
    return notImplemented || [];
  }, [selectedMethod, notImplemented, connector]);

  const [sessionName, setSessionName] = useState(() => {
    if (selectedMethod) {
      return `Implement ${selectedMethod.method} for ${connector.name}`;
    }
    return `Implement ${connector.name} Payment Flows`;
  });
  
  const [description, setDescription] = useState(() => generateDefaultDescription(connector, items, selectedMethod));
  const [sourcePath, setSourcePath] = useState("");
  const [strategy, setStrategy] = useState<SessionCopyStrategy>("git-worktree");
  const [selectedMethods, setSelectedMethods] = useState<Set<string>>(
    new Set(items.map((ni) => ni.method))
  );

  // Group not implemented by category
  const groupedByCategory = useMemo(() => {
    const groups = new Map<string, MethodGap[]>();
    for (const item of items) {
      const existing = groups.get(item.category) || [];
      existing.push(item);
      groups.set(item.category, existing);
    }
    return groups;
  }, [items]);

  const canSubmit = sessionName.trim() && sourcePath.trim() && wsConnected;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!canSubmit) return;

    const methodsList = Array.from(selectedMethods).join(", ");
    const fullDescription = isSingleMethodMode 
      ? description 
      : `${description}\n\nTarget Payment Methods:\n${methodsList}`;

    onCreateSession({
      name: sessionName.trim(),
      description: fullDescription.trim(),
      sourcePath: sourcePath.trim(),
      strategy,
    });
  };

  const toggleMethod = (method: string) => {
    const next = new Set(selectedMethods);
    if (next.has(method)) {
      next.delete(method);
    } else {
      next.add(method);
    }
    setSelectedMethods(next);
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
      <form
        onClick={(e) => e.stopPropagation()}
        onSubmit={handleSubmit}
        style={{
          width: 640,
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
            <h2 style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>
              Create Task: {connector.name}
            </h2>
            <p style={{ margin: "4px 0 0", fontSize: 12, color: T.textMuted }}>
              {isSingleMethodMode 
                ? `Implement ${selectedMethod?.method} (${selectedMethod?.category})`
                : `${items.length} not-implemented payment methods available`
              }
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            style={{
              border: "none",
              background: "transparent",
              cursor: "pointer",
              color: T.textMuted,
              fontSize: 20,
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
            padding: "20px 24px",
          }}
        >
          {!wsConnected && (
            <div
              style={{
                background: T.warnSoft,
                color: T.warn,
                padding: "10px 14px",
                borderRadius: 6,
                fontSize: 12,
                marginBottom: 16,
              }}
            >
              WebSocket not connected. Connect to supervisor before creating sessions.
            </div>
          )}

          {/* Connector Summary */}
          <div
            style={{
              background: T.bg,
              border: `1px solid ${T.border}`,
              borderRadius: 8,
              padding: 16,
              marginBottom: 20,
            }}
          >
            <div style={{ fontWeight: 600, marginBottom: 8 }}>Connector Summary</div>
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(4, 1fr)",
                gap: 12,
                fontSize: 12,
              }}
            >
              <StatBox label="Supported" value={connector.stats.supported} color={T.success} />
              <StatBox
                label="Not Implemented"
                value={connector.stats.notImplemented}
                color={T.warn}
              />
              <StatBox
                label="Not Supported"
                value={connector.stats.notSupported}
                color={T.textSubtle}
              />
              <StatBox label="Total" value={connector.stats.total} color={T.text} />
            </div>
            <div style={{ marginTop: 12, fontSize: 11, color: T.textSubtle }}>
              Documentation: {" "}
              <a href={`/docs/${connector.filePath}`} style={{ color: T.accent }}>
                {connector.filePath}
              </a>
            </div>
          </div>

          {/* Session Fields */}
          <div style={{ marginBottom: 16 }}>
            <label style={fieldLabelStyle}>Session Name *</label>
            <input
              value={sessionName}
              onChange={(e) => setSessionName(e.target.value)}
              placeholder="e.g., Implement Stripe Card & Wallet Flows"
              style={inputStyle()}
            />
          </div>

          <div style={{ marginBottom: 16 }}>
            <label style={fieldLabelStyle}>Description</label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={4}
              style={{ ...inputStyle(), resize: "vertical", fontFamily: "inherit" }}
            />
          </div>

          <div style={{ marginBottom: 16 }}>
            <label style={fieldLabelStyle}>Source Folder *</label>
            <input
              value={sourcePath}
              onChange={(e) => setSourcePath(e.target.value)}
              placeholder="/path/to/hyperswitch-prism"
              style={inputStyle()}
            />
          </div>

          <div style={{ marginBottom: 20 }}>
            <label style={fieldLabelStyle}>Copy Strategy</label>
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
                    gap: 8,
                    cursor: "pointer",
                    fontSize: 13,
                    padding: "6px 0",
                  }}
                >
                  <input
                    type="radio"
                    name="strategy"
                    checked={strategy === value}
                    onChange={() => setStrategy(value)}
                  />
                  {label}
                </label>
              ))}
            </div>
          </div>

          {/* Payment Methods Selection */}
          {!isSingleMethodMode && items.length > 0 && (
            <div>
              <div style={{ ...fieldLabelStyle, marginBottom: 12 }}>
                Select Payment Methods to Implement ({selectedMethods.size} selected)
              </div>
              <div
                style={{
                  maxHeight: 200,
                  overflowY: "auto",
                  border: `1px solid ${T.border}`,
                  borderRadius: 6,
                  padding: 12,
                  background: T.bg,
                }}
              >
                {Array.from(groupedByCategory.entries()).map(([category, items]) => (
                  <div key={category} style={{ marginBottom: 12 }}>
                    <div
                      style={{
                        fontSize: 11,
                        fontWeight: 700,
                        color: T.textSubtle,
                        textTransform: "uppercase",
                        letterSpacing: 0.5,
                        marginBottom: 6,
                      }}
                    >
                      {category}
                    </div>
                    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                      {items.map((item) => (
                        <label
                          key={item.method}
                          style={{
                            display: "flex",
                            alignItems: "center",
                            gap: 8,
                            cursor: "pointer",
                            padding: "4px 0",
                            fontSize: 13,
                          }}
                        >
                          <input
                            type="checkbox"
                            checked={selectedMethods.has(item.method)}
                            onChange={() => toggleMethod(item.method)}
                          />
                          <span>{item.method}</span>
                        </label>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div
          style={{
            padding: "16px 24px",
            borderTop: `1px solid ${T.border}`,
            display: "flex",
            justifyContent: "flex-end",
            gap: 10,
            background: T.bgSidebar,
          }}
        >
          <button
            type="button"
            onClick={onClose}
            style={{
              padding: "8px 16px",
              borderRadius: 6,
              border: `1px solid ${T.border}`,
              background: "transparent",
              color: T.text,
              fontSize: 13,
              cursor: "pointer",
            }}
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={!canSubmit}
            style={{
              padding: "8px 18px",
              borderRadius: 6,
              border: "none",
              background: canSubmit ? T.accent : T.border,
              color: "#fff",
              fontWeight: 600,
              fontSize: 13,
              cursor: canSubmit ? "pointer" : "not-allowed",
            }}
          >
            Create Session
          </button>
        </div>
      </form>
    </div>
  );
}

function StatBox({
  label,
  value,
  color,
}: {
  label: string;
  value: number;
  color: string;
}) {
  return (
    <div style={{ textAlign: "center" }}>
      <div style={{ fontSize: 18, fontWeight: 700, color }}>{value}</div>
      <div style={{ fontSize: 10, color: T.textSubtle, textTransform: "uppercase" }}>
        {label}
      </div>
    </div>
  );
}

function generateDefaultDescription(
  connector: Connector,
  items: MethodGap[],
  selectedMethod?: ConnectorPaymentMethod
): string {
  if (selectedMethod) {
    return [
      `Implement ${selectedMethod.method} payment method for ${connector.name} connector.`,
      "",
      "Details:",
      `- Connector: ${connector.name}`,
      `- Payment Method: ${selectedMethod.method}`,
      `- Category: ${selectedMethod.category}`,
      `- Status: Not Implemented`,
      "",
      `Documentation: ${connector.filePath}`,
      "",
      "Acceptance Criteria:",
      `- ${selectedMethod.method} authorization flow works correctly`,
      `- Error handling follows existing patterns`,
      `- Tests pass for the new payment method`,
    ].join("\n");
  }

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

  if (items.length > 0) {
    const byCategory = new Map<string, string[]>();
    for (const item of items) {
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

const fieldLabelStyle: React.CSSProperties = {
  display: "block",
  fontSize: 12,
  fontWeight: 600,
  color: T.textMuted,
  marginBottom: 6,
};

function inputStyle(): React.CSSProperties {
  return {
    width: "100%",
    padding: "10px 12px",
    borderRadius: 6,
    border: `1px solid ${T.border}`,
    background: T.bg,
    color: T.text,
    fontSize: 13,
    outline: "none",
    fontFamily: "inherit",
  };
}
