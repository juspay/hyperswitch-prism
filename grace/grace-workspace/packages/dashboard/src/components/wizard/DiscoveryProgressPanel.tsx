import { useEffect, useMemo, useRef } from "react";
import { T } from "../../theme";
import type { WizardState } from "./types";

export type DiscoveryEvent = {
  kind: "line" | "progress";
  text: string;
  /** Server-side timestamp. Used by IntegrationStepBasics to dedupe SSE
   * replay against the localStorage-restored backlog on resume. */
  ts?: number;
};

interface Props {
  status: WizardState["discoveryStatus"];
  events: DiscoveryEvent[];
  open: boolean;
  onToggle: () => void;
  onCancel: () => void;
  startedAt: number | null;
}

const KEYFRAMES_ID = "discovery-progress-keyframes";

function ensureKeyframes() {
  if (typeof document === "undefined") return;
  if (document.getElementById(KEYFRAMES_ID)) return;
  const style = document.createElement("style");
  style.id = KEYFRAMES_ID;
  style.textContent = `
@keyframes discovery-progress-stripe {
  0%   { transform: translateX(-40%); }
  100% { transform: translateX(140%); }
}
`;
  document.head.appendChild(style);
}

function lineColor(text: string): { color: string; fontWeight?: number; fontStyle?: string; fontSize?: number } {
  if (text.startsWith("🔧")) return { color: T.accent, fontWeight: 600 };
  if (text.startsWith("   ✓")) return { color: T.success };
  if (text.startsWith("   ✗")) return { color: T.error };
  if (text.startsWith("💬")) return { color: T.text };
  if (text.startsWith("🤔")) return { color: T.textMuted, fontStyle: "italic" };
  if (text.startsWith("▶") || text.startsWith("✓ done")) {
    return { color: T.textMuted, fontSize: 11 };
  }
  return { color: T.text };
}

function statusPill(status: WizardState["discoveryStatus"]): {
  label: string;
  bg: string;
  fg: string;
  spin?: boolean;
} {
  switch (status) {
    case "running":
      return { label: "⟳ Running", bg: T.accentSoft, fg: T.accent, spin: true };
    case "done":
      return { label: "✓ Done", bg: T.successSoft, fg: T.success };
    case "error":
      return { label: "✕ Error", bg: T.errorSoft, fg: T.error };
    case "cancelled":
      return { label: "⊘ Cancelled", bg: T.warnSoft, fg: T.warn };
    default:
      return { label: "· Idle", bg: T.bg, fg: T.textMuted };
  }
}

export function DiscoveryProgressPanel({
  status,
  events,
  open,
  onToggle,
  onCancel,
  startedAt,
}: Props) {
  ensureKeyframes();
  const listRef = useRef<HTMLDivElement | null>(null);
  const lastLine = events.length > 0 ? events[events.length - 1]!.text : "";
  const pill = statusPill(status);
  const isRunning = status === "running";

  useEffect(() => {
    if (!open) return;
    const el = listRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [events.length, open]);

  const summary = useMemo(() => {
    if (!startedAt || isRunning) return null;
    const toolCalls = events.filter((e) => e.kind === "line" && e.text.startsWith("🔧")).length;
    const seconds = Math.round((Date.now() - startedAt) / 1000);
    return `${toolCalls} tool call${toolCalls === 1 ? "" : "s"} · ${seconds}s`;
  }, [events, startedAt, isRunning]);

  return (
    <div
      style={{
        marginTop: 12,
        border: `1px solid ${T.border}`,
        borderRadius: 8,
        background: T.bgElev,
        overflow: "hidden",
      }}
    >
      {/* Header */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 10,
          padding: "8px 12px",
          background: T.bgSidebar,
          borderBottom: open ? `1px solid ${T.border}` : "none",
        }}
      >
        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 4,
            padding: "2px 8px",
            borderRadius: 999,
            background: pill.bg,
            color: pill.fg,
            fontSize: 11,
            fontWeight: 600,
            whiteSpace: "nowrap",
          }}
        >
          {pill.spin ? (
            <>
              <span style={{ animation: "spin 1s linear infinite" }}>⟳</span>
              <span>Running</span>
            </>
          ) : (
            pill.label
          )}
        </span>

        <div
          style={{
            flex: 1,
            minWidth: 0,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            fontFamily: "ui-monospace, SF Mono, Menlo, Consolas, monospace",
            fontSize: 12,
            color: T.textMuted,
          }}
          title={lastLine}
        >
          {open ? "Live agent activity" : lastLine || "Waiting for first event…"}
        </div>

        {isRunning && (
          <button
            type="button"
            onClick={onCancel}
            style={{
              background: "none",
              border: "none",
              color: T.warn,
              fontSize: 12,
              fontWeight: 600,
              padding: 0,
              cursor: "pointer",
            }}
          >
            Cancel
          </button>
        )}

        <button
          type="button"
          onClick={onToggle}
          style={{
            background: "none",
            border: "none",
            color: T.accent,
            fontSize: 12,
            fontWeight: 600,
            padding: 0,
            cursor: "pointer",
            whiteSpace: "nowrap",
          }}
        >
          {open ? "Hide ▴" : "Show ▾"}
        </button>
      </div>

      {/* Indeterminate progress bar */}
      {isRunning && (
        <div
          style={{
            position: "relative",
            height: 3,
            background: T.accentSoft,
            overflow: "hidden",
          }}
        >
          <div
            style={{
              position: "absolute",
              left: 0,
              top: 0,
              bottom: 0,
              width: "40%",
              background: T.accent,
              animation: "discovery-progress-stripe 1.4s linear infinite",
            }}
          />
        </div>
      )}

      {/* Body */}
      {open && (
        <div
          ref={listRef}
          style={{
            maxHeight: 300,
            overflowY: "auto",
            background: T.bg,
            padding: "8px 12px",
            fontFamily: "ui-monospace, SF Mono, Menlo, Consolas, monospace",
            fontSize: 12,
            lineHeight: 1.5,
          }}
        >
          {events.length === 0 ? (
            <div style={{ color: T.textMuted, fontStyle: "italic" }}>
              Waiting for the agent to start…
            </div>
          ) : (
            events.map((e, i) => {
              if (e.kind === "progress") {
                return (
                  <div
                    key={i}
                    style={{
                      color: T.warn,
                      fontWeight: 600,
                    }}
                  >
                    · {e.text}
                  </div>
                );
              }
              const style = lineColor(e.text);
              return (
                <div
                  key={i}
                  style={{
                    color: style.color,
                    fontWeight: style.fontWeight,
                    fontStyle: style.fontStyle,
                    fontSize: style.fontSize,
                    whiteSpace: "pre-wrap",
                    wordBreak: "break-word",
                  }}
                >
                  {e.text}
                </div>
              );
            })
          )}
        </div>
      )}

      {/* Footer summary */}
      {summary && (
        <div
          style={{
            padding: "6px 12px",
            background: T.bgSidebar,
            borderTop: `1px solid ${T.border}`,
            fontSize: 11,
            color: T.textMuted,
          }}
        >
          {summary}
        </div>
      )}
    </div>
  );
}
