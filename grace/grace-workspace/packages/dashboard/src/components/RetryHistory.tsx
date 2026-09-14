import { useState, useRef, useEffect } from "react";
import { T } from "../theme";

export interface RetryAttempt {
  attempt: number;
  status: "passed" | "failed" | "running";
  timestamp: string;
}

interface RetryHistoryProps {
  currentAttempt: number;
  attempts: RetryAttempt[];
  selectedAttempt: number;
  onSelectAttempt: (attempt: number) => void;
  onBackToCurrent: () => void;
}

export function RetryHistory({
  currentAttempt,
  attempts,
  selectedAttempt,
  onSelectAttempt,
  onBackToCurrent,
}: RetryHistoryProps) {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Close dropdown when clicking outside
  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const isViewingHistorical = selectedAttempt !== currentAttempt;
  const selectedInfo = attempts.find((a) => a.attempt === selectedAttempt);

  const getStatusDot = (status: string) => {
    switch (status) {
      case "passed":
        return { bg: T.success, label: "Passed" };
      case "failed":
        return { bg: T.error, label: "Failed" };
      case "running":
        return { bg: T.accent, label: "Running" };
      default:
        return { bg: T.textMuted, label: "Unknown" };
    }
  };

  const formatTimeAgo = (timestamp: string) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);

    if (diffMins < 1) return "just now";
    if (diffMins < 60) return `${diffMins} min${diffMins !== 1 ? "s" : ""} ago`;
    if (diffHours < 24) return `${diffHours} hour${diffHours !== 1 ? "s" : ""} ago`;
    return date.toLocaleDateString();
  };

  return (
    <div>
      {/* Historical View Banner */}
      {isViewingHistorical && (
        <div
          style={{
            padding: "12px 16px",
            background: T.warnSoft,
            border: `1px solid ${T.warn}`,
            borderRadius: 8,
            marginBottom: 16,
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            flexWrap: "wrap",
            gap: 12,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <span style={{ fontSize: 18 }}>⚠️</span>
            <div>
              <div
                style={{
                  fontSize: 13,
                  fontWeight: 700,
                  color: T.warn,
                }}
              >
                Viewing Attempt #{selectedAttempt} of {currentAttempt} (Historical)
              </div>
              {selectedInfo && (
                <div style={{ fontSize: 11, color: T.textMuted, marginTop: 2 }}>
                  {formatTimeAgo(selectedInfo.timestamp)} •{" "}
                  {getStatusDot(selectedInfo.status).label}
                </div>
              )}
            </div>
          </div>
          <button
            onClick={onBackToCurrent}
            style={{
              padding: "6px 12px",
              borderRadius: 6,
              border: `1px solid ${T.warn}`,
              background: "transparent",
              color: T.warn,
              fontSize: 12,
              fontWeight: 600,
              cursor: "pointer",
              whiteSpace: "nowrap",
            }}
          >
            ← Back to Current
          </button>
        </div>
      )}

      {/* Retry Selector Dropdown */}
      {attempts.length > 1 && (
        <div
          ref={dropdownRef}
          style={{
            position: "relative",
            display: "inline-block",
            marginBottom: isViewingHistorical ? 0 : 16,
          }}
        >
          <button
            onClick={() => setIsOpen(!isOpen)}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              padding: "8px 14px",
              borderRadius: 6,
              border: `1px solid ${T.borderStrong}`,
              background: T.bgElev,
              color: T.text,
              fontSize: 13,
              fontWeight: 600,
              cursor: "pointer",
              minWidth: 180,
            }}
          >
            <span
              style={{
                width: 8,
                height: 8,
                borderRadius: "50%",
                background: getStatusDot(selectedInfo?.status || "unknown").bg,
              }}
            />
            <span style={{ flex: 1, textAlign: "left" }}>
              Attempt {selectedAttempt} of {currentAttempt}
              {selectedAttempt === currentAttempt && " (Current)"}
            </span>
            <span style={{ color: T.textMuted, fontSize: 10 }}>
              {isOpen ? "▲" : "▼"}
            </span>
          </button>

          {isOpen && (
            <div
              style={{
                position: "absolute",
                top: "100%",
                left: 0,
                right: 0,
                marginTop: 4,
                background: T.bgElev,
                border: `1px solid ${T.borderStrong}`,
                borderRadius: 6,
                boxShadow: "0 4px 12px rgba(0,0,0,0.3)",
                zIndex: 100,
                maxHeight: 300,
                overflow: "auto",
              }}
            >
              {[...attempts]
                .sort((a, b) => b.attempt - a.attempt)
                .map((attempt) => {
                  const isSelected = attempt.attempt === selectedAttempt;
                  const isCurrent = attempt.attempt === currentAttempt;
                  const statusDot = getStatusDot(attempt.status);

                  return (
                    <button
                      key={attempt.attempt}
                      onClick={() => {
                        onSelectAttempt(attempt.attempt);
                        setIsOpen(false);
                      }}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 10,
                        width: "100%",
                        padding: "10px 14px",
                        border: "none",
                        borderBottom: `1px solid ${T.border}`,
                        background: isSelected ? T.accentSoft : "transparent",
                        color: T.text,
                        fontSize: 13,
                        cursor: "pointer",
                        textAlign: "left",
                        transition: "background 150ms",
                      }}
                      onMouseEnter={(e) => {
                        if (!isSelected) {
                          e.currentTarget.style.background = T.codeBg;
                        }
                      }}
                      onMouseLeave={(e) => {
                        if (!isSelected) {
                          e.currentTarget.style.background = "transparent";
                        }
                      }}
                    >
                      <span
                        style={{
                          width: 8,
                          height: 8,
                          borderRadius: "50%",
                          background: statusDot.bg,
                          flexShrink: 0,
                        }}
                      />
                      <span style={{ flex: 1 }}>
                        Attempt {attempt.attempt}
                        {isCurrent && (
                          <span
                            style={{
                              marginLeft: 6,
                              padding: "2px 6px",
                              borderRadius: 4,
                              background: T.accentSoft,
                              color: T.accent,
                              fontSize: 10,
                              fontWeight: 700,
                            }}
                          >
                            CURRENT
                          </span>
                        )}
                      </span>
                      <span
                        style={{
                          fontSize: 11,
                          color: T.textMuted,
                          whiteSpace: "nowrap",
                        }}
                      >
                        {formatTimeAgo(attempt.timestamp)}
                      </span>
                    </button>
                  );
                })}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
