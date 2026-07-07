import { useEffect, useRef, useState } from "react";
import type { LogLine } from "../hooks/usePipeline";
import { T } from "../theme";

const LOG_COLOR: Record<string, string> = {
  info: "#a0522d",
  warn: "#c2670a",
  error: "#b91c1c",
  success: "#65a30d",
  debug: "#b89874",
};

export function LogPanel({ logs }: { logs: LogLine[] }) {
  const [autoScroll, setAutoScroll] = useState(true);
  const ref = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (autoScroll && ref.current) {
      ref.current.scrollTop = ref.current.scrollHeight;
    }
  }, [logs, autoScroll]);

  const onScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 20;
    setAutoScroll(atBottom);
  };

  return (
    <aside
      style={{
        width: 360,
        background: T.bgRight,
        borderLeft: `1px solid ${T.border}`,
        display: "flex",
        flexDirection: "column",
        height: "100vh",
        flexShrink: 0,
      }}
    >
      <div
        style={{
          padding: "20px 22px 16px",
          borderBottom: `1px solid ${T.border}`,
          background: T.bgRightHeader,
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 12,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <div
            style={{
              width: 8,
              height: 8,
              borderRadius: "50%",
              background: T.success,
              boxShadow: `0 0 0 3px ${T.successSoft}`,
              animation: "pulse 1.8s ease-in-out infinite",
            }}
          />
          <div>
            <div
              style={{
                fontSize: 13,
                fontWeight: 700,
                color: T.text,
                letterSpacing: 0.2,
              }}
            >
              Live logs
            </div>
            <div
              style={{
                fontSize: 11,
                color: T.textMuted,
                marginTop: 2,
                fontVariantNumeric: "tabular-nums",
              }}
            >
              {logs.length} {logs.length === 1 ? "entry" : "entries"}
            </div>
          </div>
        </div>
        {!autoScroll && (
          <button
            onClick={() => setAutoScroll(true)}
            style={{
              background: T.bgElev,
              border: `1px solid ${T.borderStrong}`,
              color: T.accent,
              fontSize: 10,
              fontWeight: 600,
              cursor: "pointer",
              padding: "5px 10px",
              borderRadius: 6,
              textTransform: "uppercase",
              letterSpacing: 0.4,
            }}
          >
            ↓ bottom
          </button>
        )}
      </div>
      <div
        ref={ref}
        onScroll={onScroll}
        style={{
          flex: 1,
          overflowY: "auto",
          padding: "10px 16px 16px",
          fontFamily: "ui-monospace, SFMono-Regular, monospace",
          fontSize: 11,
          lineHeight: 1.55,
        }}
      >
        {logs.length === 0 && (
          <div style={{ color: T.textSubtle, fontStyle: "italic", paddingTop: 8 }}>
            No logs yet.
          </div>
        )}
        {logs.map((l, i) => (
          <div
            key={i}
            style={{
              color: LOG_COLOR[l.level] ?? T.text,
              marginBottom: 2,
              wordBreak: "break-word",
            }}
          >
            <span style={{ color: T.textSubtle }}>{l.ts.slice(11, 19)} </span>
            {l.checkpointId && (
              <span style={{ color: T.textMuted }}>[{l.checkpointId}] </span>
            )}
            {l.msg}
          </div>
        ))}
      </div>
    </aside>
  );
}
