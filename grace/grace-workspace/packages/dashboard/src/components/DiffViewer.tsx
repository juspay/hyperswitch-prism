import { useEffect, useMemo, useState } from "react";
import { T } from "../theme";

interface Props {
  diff: string;
  collapsedByDefault?: boolean;
  /**
   * Below this container width the viewer forces inline mode regardless of
   * the user's toggle preference — same trick GitHub uses on narrow viewports.
   */
  splitMinWidth?: number;
}

/**
 * Unified-diff renderer with toggleable split / inline modes:
 *
 *   - **Inline**: classic unified view, one column, +/- coloured lines.
 *   - **Split**: side-by-side. Each `@@` hunk is walked once; consecutive `-`
 *     runs and consecutive `+` runs get paired up index-wise so a 3-removed,
 *     2-added run renders as 3 rows with the last row's right cell blank.
 *     Context lines mirror on both sides.
 *
 * Auto-collapses to inline below `splitMinWidth` (default 960px) — the table
 * gets unreadable on narrow screens. The user's toggle preference is kept in
 * component state, so resizing back up restores the split view.
 */
const COLLAPSE_THRESHOLD_LINES = 400;
const DEFAULT_SPLIT_MIN_WIDTH = 960;

type Mode = "inline" | "split";

export function DiffViewer({
  diff,
  collapsedByDefault = true,
  splitMinWidth = DEFAULT_SPLIT_MIN_WIDTH,
}: Props) {
  const [expanded, setExpanded] = useState(!collapsedByDefault);
  const [userMode, setUserMode] = useState<Mode>("split");
  const [wideEnough, setWideEnough] = useState(true);

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return;
    }
    const mql = window.matchMedia(`(min-width: ${splitMinWidth}px)`);
    const update = () => setWideEnough(mql.matches);
    update();
    mql.addEventListener("change", update);
    return () => mql.removeEventListener("change", update);
  }, [splitMinWidth]);

  const effectiveMode: Mode = wideEnough ? userMode : "inline";

  const lines = useMemo(() => diff.split("\n"), [diff]);
  const visible = useMemo(() => {
    if (expanded) return lines;
    return lines.slice(0, COLLAPSE_THRESHOLD_LINES);
  }, [lines, expanded]);
  const hidden = lines.length - visible.length;

  const splitRows = useMemo(
    () => (effectiveMode === "split" ? buildSplitRows(visible) : []),
    [effectiveMode, visible]
  );

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
          gap: 10,
          fontSize: 11,
          color: T.textMuted,
          flexWrap: "wrap",
        }}
      >
        <span>
          {lines.length} line{lines.length === 1 ? "" : "s"} ·{" "}
          {countByPrefix(lines, "+")} added · {countByPrefix(lines, "-")} removed
        </span>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <ModeToggle
            value={userMode}
            onChange={setUserMode}
            forcedInline={!wideEnough}
          />
          {hidden > 0 && (
            <button
              type="button"
              onClick={() => setExpanded(true)}
              style={pillButton}
            >
              Show {hidden} more
            </button>
          )}
          {expanded && lines.length > COLLAPSE_THRESHOLD_LINES && (
            <button
              type="button"
              onClick={() => setExpanded(false)}
              style={pillButton}
            >
              Collapse
            </button>
          )}
        </div>
      </div>
      {effectiveMode === "inline" ? (
        <InlineDiff lines={visible} />
      ) : (
        <SplitDiff rows={splitRows} />
      )}
    </div>
  );
}

// ─── Inline mode ─────────────────────────────────────────────────────────

function InlineDiff({ lines }: { lines: string[] }) {
  return (
    <pre
      data-diff-mode="inline"
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
      {lines.map((line, i) => (
        <div key={i} style={inlineLineStyle(line)}>
          {line || "\u00A0"}
        </div>
      ))}
    </pre>
  );
}

function inlineLineStyle(line: string): React.CSSProperties {
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

// ─── Split mode ──────────────────────────────────────────────────────────

interface SplitRow {
  kind: "context" | "change" | "header" | "hunk";
  left: string | null;
  right: string | null;
}

function buildSplitRows(lines: string[]): SplitRow[] {
  const rows: SplitRow[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i]!;
    if (isFileHeader(line)) {
      rows.push({ kind: "header", left: line, right: line });
      i++;
      continue;
    }
    if (line.startsWith("@@")) {
      rows.push({ kind: "hunk", left: line, right: line });
      i++;
      continue;
    }
    if (isRemoval(line)) {
      const minuses: string[] = [];
      while (i < lines.length && isRemoval(lines[i]!)) {
        minuses.push(lines[i]!);
        i++;
      }
      const pluses: string[] = [];
      while (i < lines.length && isAddition(lines[i]!)) {
        pluses.push(lines[i]!);
        i++;
      }
      const maxLen = Math.max(minuses.length, pluses.length);
      for (let k = 0; k < maxLen; k++) {
        rows.push({
          kind: "change",
          left: minuses[k] ?? null,
          right: pluses[k] ?? null,
        });
      }
      continue;
    }
    if (isAddition(line)) {
      while (i < lines.length && isAddition(lines[i]!)) {
        rows.push({ kind: "change", left: null, right: lines[i]! });
        i++;
      }
      continue;
    }
    rows.push({ kind: "context", left: line, right: line });
    i++;
  }
  return rows;
}

function isFileHeader(line: string): boolean {
  return (
    line.startsWith("diff ") ||
    line.startsWith("index ") ||
    line.startsWith("+++ ") ||
    line.startsWith("--- ") ||
    line.startsWith("new file mode") ||
    line.startsWith("deleted file mode") ||
    line.startsWith("similarity index") ||
    line.startsWith("rename from") ||
    line.startsWith("rename to")
  );
}

function isAddition(line: string): boolean {
  return line.startsWith("+") && !line.startsWith("+++");
}

function isRemoval(line: string): boolean {
  return line.startsWith("-") && !line.startsWith("---");
}

function SplitDiff({ rows }: { rows: SplitRow[] }) {
  return (
    <div
      data-diff-mode="split"
      style={{
        maxHeight: 560,
        overflow: "auto",
        background: T.bg,
        fontSize: 12,
        fontFamily:
          "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
        lineHeight: 1.5,
      }}
    >
      <table
        style={{
          width: "100%",
          borderCollapse: "collapse",
          tableLayout: "fixed",
        }}
      >
        <colgroup>
          <col style={{ width: "50%" }} />
          <col style={{ width: "50%" }} />
        </colgroup>
        <tbody>
          {rows.map((row, i) => {
            if (row.kind === "header") {
              return (
                <tr key={i}>
                  <td colSpan={2} style={splitHeaderCell}>
                    {row.left || "\u00A0"}
                  </td>
                </tr>
              );
            }
            if (row.kind === "hunk") {
              return (
                <tr key={i}>
                  <td colSpan={2} style={splitHunkCell}>
                    {row.left || "\u00A0"}
                  </td>
                </tr>
              );
            }
            return (
              <tr key={i}>
                <td style={splitCellStyle(row.left, "left", row.kind)}>
                  {stripPrefix(row.left) ?? "\u00A0"}
                </td>
                <td style={splitCellStyle(row.right, "right", row.kind)}>
                  {stripPrefix(row.right) ?? "\u00A0"}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function stripPrefix(line: string | null): string | null {
  if (line === null) return null;
  if (line.length === 0) return "";
  const first = line[0];
  if (first === "+" || first === "-" || first === " ") return line.slice(1);
  return line;
}

function splitCellStyle(
  line: string | null,
  side: "left" | "right",
  kind: SplitRow["kind"]
): React.CSSProperties {
  const base: React.CSSProperties = {
    padding: "0 10px",
    verticalAlign: "top",
    whiteSpace: "pre-wrap",
    wordBreak: "break-all",
    borderRight: side === "left" ? `1px solid ${T.border}` : undefined,
    color: T.text,
  };
  if (line === null) {
    return { ...base, background: T.bgElev, opacity: 0.5 };
  }
  if (kind === "change") {
    if (side === "left") {
      return { ...base, background: T.errorSoft, color: T.error };
    }
    return { ...base, background: T.successSoft, color: T.success };
  }
  return base;
}

const splitHeaderCell: React.CSSProperties = {
  padding: "0 10px",
  background: T.bgElev,
  color: T.textMuted,
  fontWeight: 600,
  whiteSpace: "pre-wrap",
  wordBreak: "break-all",
};

const splitHunkCell: React.CSSProperties = {
  padding: "0 10px",
  background: T.accentSoft,
  color: T.accent,
  whiteSpace: "pre-wrap",
  wordBreak: "break-all",
};

// ─── Mode toggle ─────────────────────────────────────────────────────────

function ModeToggle({
  value,
  onChange,
  forcedInline,
}: {
  value: Mode;
  onChange: (next: Mode) => void;
  forcedInline: boolean;
}) {
  return (
    <div
      data-testid="diff-mode-toggle"
      style={{
        display: "inline-flex",
        border: `1px solid ${T.border}`,
        borderRadius: 4,
        overflow: "hidden",
      }}
      title={
        forcedInline
          ? "Viewport too narrow for split view — resize wider to enable"
          : "Switch between inline and split diff views"
      }
    >
      <ToggleSegment
        active={value === "inline" || forcedInline}
        disabled={false}
        onClick={() => onChange("inline")}
      >
        Inline
      </ToggleSegment>
      <ToggleSegment
        active={value === "split" && !forcedInline}
        disabled={forcedInline}
        onClick={() => onChange("split")}
      >
        Split
      </ToggleSegment>
    </div>
  );
}

function ToggleSegment({
  active,
  disabled,
  onClick,
  children,
}: {
  active: boolean;
  disabled: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      style={{
        fontSize: 11,
        padding: "3px 10px",
        border: "none",
        background: active ? T.accent : T.bg,
        color: active ? "#fff" : disabled ? T.textSubtle : T.text,
        cursor: disabled ? "not-allowed" : "pointer",
        fontWeight: active ? 600 : 500,
      }}
    >
      {children}
    </button>
  );
}

// ─── Shared bits ─────────────────────────────────────────────────────────

const pillButton: React.CSSProperties = {
  fontSize: 11,
  padding: "3px 10px",
  borderRadius: 4,
  border: `1px solid ${T.border}`,
  background: T.bg,
  color: T.text,
  cursor: "pointer",
};

function countByPrefix(lines: string[], prefix: string): number {
  // Skip diff header / file marker prefixes that just happen to start with +/-.
  let count = 0;
  for (const line of lines) {
    if (line.startsWith("+++") || line.startsWith("---")) continue;
    if (line.startsWith(prefix)) count++;
  }
  return count;
}
