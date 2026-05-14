import { useEffect, useMemo, useState } from "react";
import { T } from "../theme";

interface Props {
  diff: string;
  collapsedByDefault?: boolean;
}

/**
 * Minimal unified-diff renderer — no external dep. Splits into hunks by
 * `@@ … @@` headers; colours each line by leading char (+ green, - red,
 * everything else neutral). For big diffs we hide content beyond a
 * threshold and offer a "show all" toggle.
 */
const COLLAPSE_THRESHOLD_LINES = 400;

export function DiffViewer({ diff, collapsedByDefault = true }: Props) {
  const [expanded, setExpanded] = useState(!collapsedByDefault);
  const lines = useMemo(() => diff.split("\n"), [diff]);
  const visible = useMemo(() => {
    if (expanded) return lines;
    return lines.slice(0, COLLAPSE_THRESHOLD_LINES);
  }, [lines, expanded]);
  const hidden = lines.length - visible.length;

  // Re-collapse if a brand-new diff arrives
  useEffect(() => {
    setExpanded(!collapsedByDefault);
  }, [diff, collapsedByDefault]);

  if (!diff.trim()) {
    return (
      <div
        style={{
          padding: 16,
          fontSize: 12,
          color: T.textMuted,
          fontStyle: "italic",
          background: T.bg,
          borderRadius: 6,
          border: `1px dashed ${T.border}`,
        }}
      >
        No diff captured yet.
      </div>
    );
  }

  return (
    <div
      style={{
        border: `1px solid ${T.border}`,
        borderRadius: 8,
        overflow: "hidden",
        background: T.bg,
        boxShadow: T.shadow,
      }}
    >
      <div
        style={{
          padding: "8px 14px",
          borderBottom: `1px solid ${T.border}`,
          background: T.bgElev,
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          fontSize: 11,
          color: T.textMuted,
        }}
      >
        <span>
          {lines.length} line{lines.length === 1 ? "" : "s"} ·{" "}
          {countByPrefix(lines, "+")} added · {countByPrefix(lines, "-")} removed
        </span>
        {hidden > 0 && (
          <button
            type="button"
            onClick={() => setExpanded(true)}
            style={{
              fontSize: 11,
              padding: "3px 10px",
              borderRadius: 4,
              border: `1px solid ${T.border}`,
              background: T.bg,
              color: T.text,
              cursor: "pointer",
            }}
          >
            Show {hidden} more
          </button>
        )}
        {expanded && lines.length > COLLAPSE_THRESHOLD_LINES && (
          <button
            type="button"
            onClick={() => setExpanded(false)}
            style={{
              fontSize: 11,
              padding: "3px 10px",
              borderRadius: 4,
              border: `1px solid ${T.border}`,
              background: T.bg,
              color: T.text,
              cursor: "pointer",
            }}
          >
            Collapse
          </button>
        )}
      </div>
      <pre
        style={{
          margin: 0,
          padding: 0,
          fontSize: 12,
          fontFamily:
            "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
          lineHeight: 1.5,
          maxHeight: 560,
          overflow: "auto",
          background: T.bg,
        }}
      >
        {visible.map((line, i) => (
          <div key={i} style={lineStyle(line)}>
            {line || "\u00A0"}
          </div>
        ))}
      </pre>
    </div>
  );
}

function lineStyle(line: string): React.CSSProperties {
  const base: React.CSSProperties = {
    padding: "0 14px",
    whiteSpace: "pre-wrap",
    wordBreak: "break-all",
  };
  if (line.startsWith("+++") || line.startsWith("---")) {
    return { ...base, color: T.text, fontWeight: 600, background: T.bgElev };
  }
  if (line.startsWith("@@")) {
    return { ...base, color: T.accent, background: T.accentSoft };
  }
  if (line.startsWith("+")) {
    return { ...base, color: T.success, background: T.successSoft };
  }
  if (line.startsWith("-")) {
    return { ...base, color: T.error, background: T.errorSoft };
  }
  if (line.startsWith("diff ")) {
    return { ...base, color: T.textMuted, background: T.bgElev, fontWeight: 600 };
  }
  return { ...base, color: T.text };
}

function countByPrefix(lines: string[], prefix: string): number {
  // Skip diff header / file marker prefixes that just happen to start with +/-.
  let count = 0;
  for (const line of lines) {
    if (line.startsWith("+++") || line.startsWith("---")) continue;
    if (line.startsWith(prefix)) count++;
  }
  return count;
}
