import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { SidebarLayout } from "../components/NavigationSidebar";
import { CELL_STYLE, Legend, ProgressBar, StatCard } from "../components/mvp/primitives";
import { T } from "../theme";
import type { MvpConnector } from "../types/mvp";
import { useMvpData } from "../lib/useMvpData";

/** Readiness bands. A connector is "at MVP" only with zero scored gaps. */
const BANDS = [
  { id: "complete", label: "At MVP", test: (c: MvpConnector) => c.gaps === 0 },
  { id: "close", label: "Close · 1–3 gaps", test: (c: MvpConnector) => c.gaps >= 1 && c.gaps <= 3 },
  { id: "progress", label: "In progress · 4–8 gaps", test: (c: MvpConnector) => c.gaps >= 4 && c.gaps <= 8 },
  { id: "early", label: "Early · 9+ gaps", test: (c: MvpConnector) => c.gaps >= 9 },
] as const;

function bandOf(c: MvpConnector) {
  return BANDS.find((b) => b.test(c))!.id;
}

function titleCase(name: string) {
  return name.replace(/_/g, " ").replace(/\b\w/g, (ch) => ch.toUpperCase());
}

export function MvpReadinessPage() {
  const { data: DATA, refreshedAt, error, loading, refresh } = useMvpData();
  const [query, setQuery] = useState("");
  const [band, setBand] = useState<string>("all");
  const [selected, setSelected] = useState<string | null>(null);

  const { capabilities, connectors } = DATA;

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return connectors.filter(
      (c) => (!q || c.name.includes(q)) && (band === "all" || bandOf(c) === band),
    );
  }, [query, band, connectors]);

  const totals = useMemo(() => {
    const atMvp = connectors.filter((c) => c.gaps === 0).length;
    const pcts = [...connectors].map((c) => c.pct).sort((a, b) => a - b);
    const median = pcts.length ? pcts[Math.floor(pcts.length / 2)] : 0;
    return {
      atMvp,
      median,
      gaps: connectors.reduce((s, c) => s + c.gaps, 0),
      effort: connectors.reduce((s, c) => s + c.effort, 0),
    };
  }, [connectors]);

  // Derived from the data, so the legend can never advertise a state the matrix
  // does not contain — every cell is a provable met/gap today.
  const presentStates = useMemo(() => {
    const seen = new Set<string>();
    for (const c of connectors) for (const v of Object.values(c.cells)) seen.add(v);
    return (["met", "gap", "unknown", "alias"] as const).filter((st) => seen.has(st));
  }, [connectors]);

  const detail = selected ? connectors.find((c) => c.name === selected) ?? null : null;

  return (
    <SidebarLayout>
      <div
        style={{
          minHeight: "100vh",
          background: T.bg,
          color: T.text,
          fontFamily: "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
        }}
      >
        <style>{`
          html, body, #root { margin: 0; padding: 0; background: ${T.bg}; }
          * { box-sizing: border-box; }
          /* The matrix scrolls inside its panel; the page never scrolls sideways,
             which would drag the sticky connector column out of view. */
          body { overflow-x: hidden; }
          .mvp-scroll::-webkit-scrollbar { height: 10px; width: 10px; }
          .mvp-scroll::-webkit-scrollbar-thumb { background: ${T.borderStrong}; border-radius: 6px; }
          .mvp-row:hover td { background: ${T.accentSoft} !important; }
        `}</style>

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
            <h1 style={{ margin: 0, fontSize: 18, fontWeight: 700 }}>MVP Readiness</h1>
            <span style={{ fontSize: 12, color: T.textMuted }}>
              {connectors.length} connectors × {capabilities.filter((c) => c.confidence === "proven").length} proven capabilities · derived from UCS Rust source · {capabilities.filter((c) => c.confidence === "best-effort").length} best-effort columns shown but not scored
            </span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
            <Link
              to="/mvp/metrics"
              style={{
                fontSize: 12.5,
                fontWeight: 600,
                color: T.accent,
                textDecoration: "none",
                border: `1px solid ${T.border}`,
                borderRadius: 6,
                padding: "8px 14px",
                background: T.bg,
              }}
            >
              📊 Metrics
            </Link>
            <RefreshButton loading={loading} onClick={refresh} />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search connectors…"
              style={{
                padding: "8px 12px",
                borderRadius: 6,
                border: `1px solid ${T.border}`,
                background: T.bg,
                color: T.text,
                fontSize: 13,
                width: 210,
                outline: "none",
              }}
            />
          </div>
        </header>

        <div style={{ padding: "20px 32px 40px", display: "flex", flexDirection: "column", gap: 18, minWidth: 0 }}>
          {/* KPI strip */}
          <div style={{ display: "flex", gap: 14, flexWrap: "wrap" }}>
            <StatCard
              label="At MVP"
              value={`${totals.atMvp} / ${connectors.length}`}
              tone={totals.atMvp === 0 ? "bad" : "good"}
              hint="zero scored gaps"
            />
            <StatCard label="Median completeness" value={`${totals.median}%`} hint="across all connectors" />
            <StatCard label="Total gaps" value={totals.gaps} tone="warn" hint="capability cells to close" />
            <StatCard label="Effort remaining" value={`${totals.effort} pts`} hint="weighted by rubric" />
          </div>

          {/* Band filters */}
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
            {[{ id: "all", label: `All · ${connectors.length}` }, ...BANDS.map((b) => ({
              id: b.id,
              label: `${b.label} · ${connectors.filter((c) => bandOf(c) === b.id).length}`,
            }))].map((b) => {
              const active = band === b.id;
              return (
                <button
                  key={b.id}
                  onClick={() => setBand(b.id)}
                  style={{
                    padding: "7px 13px",
                    borderRadius: 6,
                    fontSize: 12,
                    fontWeight: active ? 600 : 400,
                    cursor: "pointer",
                    background: active ? T.accentSoft : T.bgElev,
                    color: active ? T.accent : T.textMuted,
                    border: `1px solid ${active ? T.accent : T.border}`,
                  }}
                >
                  {b.label}
                </button>
              );
            })}
            <div style={{ marginLeft: "auto" }}>
              <Legend states={presentStates} />
            </div>
          </div>

          {/* Matrix */}
          <div
            className="mvp-scroll"
            style={{
              background: T.bgElev,
              border: `1px solid ${T.border}`,
              borderRadius: 10,
              boxShadow: T.shadow,
              overflow: "auto",
              // Panel spans the full width to line up with the KPI cards above;
              // the table inside keeps its natural column widths (max-content)
              // and simply sits left of any leftover space.
              width: "100%",
              maxHeight: "calc(100vh - 320px)",
            }}
          >
            <table
              style={{
                borderCollapse: "separate",
                borderSpacing: 0,
                fontSize: 12,
                // Natural column widths, never stretched. With minWidth:100% a
                // wide viewport dumped all the slack into the payment-method
                // columns, spreading them apart while the capability columns
                // stayed cramped. The panel hugs the table instead (below).
                width: "max-content",
              }}
            >
              <thead>
                <tr>
                  <th
                    style={{
                      ...thBase,
                      left: 0,
                      zIndex: 3,
                      minWidth: 190,
                      width: "1%",
                      whiteSpace: "nowrap",
                      textAlign: "left",
                      paddingLeft: 16,
                    }}
                  >
                    Connector
                  </th>
                  <th style={{ ...thBase, zIndex: 2, minWidth: 132, width: "1%", whiteSpace: "nowrap" }}>Readiness</th>
                  {capabilities.map((cap, i) => (
                    <th
                      key={cap.id}
                      title={
                        cap.confidence === "best-effort"
                          ? `${cap.label} — BEST EFFORT, not scored.\n\n${cap.description}`
                          : `${cap.description}\n\nweight: ${cap.weight}`
                      }
                      style={{
                        ...thBase,
                        zIndex: 2,
                        minWidth: 32,
                        padding: 0,
                        // Mark where provable data ends, so the two tiers are never
                        // read as one scale.
                        borderLeft:
                          cap.confidence === "best-effort" &&
                          capabilities[i - 1]?.confidence !== "best-effort"
                            ? `2px solid ${T.borderStrong}`
                            : undefined,
                      }}
                    >
                      <div
                        style={{
                          writingMode: "vertical-rl",
                          transform: "rotate(180deg)",
                          height: 196,
                          margin: "0 auto",
                          fontSize: 11,
                          fontWeight: 600,
                          color: cap.confidence === "best-effort" ? T.textSubtle : T.textMuted,
                          fontStyle: cap.confidence === "best-effort" ? "italic" : "normal",
                          whiteSpace: "nowrap",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          padding: "8px 0",
                        }}
                      >
                        {cap.label}
                      </div>
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.map((c) => (
                  <tr
                    key={c.name}
                    className="mvp-row"
                    onClick={() => setSelected(c.name === selected ? null : c.name)}
                    style={{ cursor: "pointer" }}
                  >
                    <td
                      style={{
                        ...tdBase,
                        position: "sticky",
                        left: 0,
                        zIndex: 1,
                        background: c.name === selected ? T.accentSoft : T.bgElev,
                        borderRight: `1px solid ${T.border}`,
                        fontWeight: 600,
                        paddingLeft: 16,
                        whiteSpace: "nowrap",
                      }}
                    >
                      <span style={{ display: "inline-flex", alignItems: "center", gap: 7 }}>
                        {titleCase(c.name)}
                      </span>
                    </td>
                    <td style={{ ...tdBase, background: T.bgElev }}>
                      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <ProgressBar pct={c.pct} />
                        <span style={{ fontWeight: 700, width: 32 }}>{c.pct}%</span>
                        <span style={{ color: T.textSubtle, fontSize: 11 }}>{c.effort}p</span>
                      </div>
                    </td>
                    {capabilities.map((cap, i) => {
                      const state = c.cells[cap.id];
                      const s = CELL_STYLE[state];
                      const soft = cap.confidence === "best-effort";
                      return (
                        <td
                          key={cap.id}
                          title={
                            `${titleCase(c.name)} · ${cap.label}: ${s.label}` +
                            (soft ? " — best effort, not scored" : "")
                          }
                          style={{
                            ...tdBase,
                            background: s.bg,
                            color: s.fg,
                            textAlign: "center",
                            fontWeight: 700,
                            // Best-effort cells read at lower contrast so the eye
                            // lands on the provable columns first.
                            opacity: soft ? 0.55 : 1,
                            borderRight: `1px solid ${T.bg}`,
                            borderLeft:
                              soft && capabilities[i - 1]?.confidence !== "best-effort"
                                ? `2px solid ${T.borderStrong}`
                                : undefined,
                          }}
                        >
                          {s.mark}
                        </td>
                      );
                    })}
                  </tr>
                ))}
                {rows.length === 0 && (
                  <tr>
                    <td colSpan={capabilities.length + 3} style={{ ...tdBase, padding: 30, textAlign: "center", color: T.textMuted }}>
                      No connectors match this filter.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>

          <div style={{ fontSize: 11, color: T.textSubtle }}>
            Derived {new Date(refreshedAt || DATA.generatedAt).toLocaleString()} · regenerated from
            source on load{error ? " · last refresh failed, showing previous data" : ""}.
          </div>
        </div>

        {/* Drill-down drawer */}
        {detail && (
          <aside
            style={{
              position: "fixed",
              top: 0,
              right: 0,
              width: 400,
              height: "100vh",
              background: T.bgElev,
              borderLeft: `1px solid ${T.borderStrong}`,
              boxShadow: T.shadowLg,
              zIndex: 40,
              display: "flex",
              flexDirection: "column",
            }}
          >
            <header
              style={{
                padding: "18px 20px",
                borderBottom: `1px solid ${T.border}`,
                display: "flex",
                justifyContent: "space-between",
                alignItems: "flex-start",
                gap: 10,
              }}
            >
              <div>
                <h2 style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>{titleCase(detail.name)}</h2>
                <div style={{ fontSize: 12, color: T.textMuted, marginTop: 4, display: "flex", alignItems: "center", gap: 7 }}>
                  {detail.pct}% · {detail.met}/{detail.scored} scored · {detail.effort} effort pts
                </div>
              </div>
              <button
                onClick={() => setSelected(null)}
                style={{
                  border: `1px solid ${T.border}`,
                  background: T.bg,
                  borderRadius: 6,
                  cursor: "pointer",
                  fontSize: 14,
                  color: T.textMuted,
                  padding: "3px 9px",
                }}
              >
                ✕
              </button>
            </header>
            <div style={{ overflowY: "auto", padding: 20, display: "flex", flexDirection: "column", gap: 18 }}>
              {/* Implemented gRPC flows, extracted from Rust source. This is the
                  evidence behind every scored cell above, so it belongs here. */}
              <div>
                <div
                  style={{
                    fontSize: 10.5,
                    fontWeight: 700,
                    textTransform: "uppercase",
                    letterSpacing: 0.5,
                    color: T.accent,
                    marginBottom: 8,
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                  }}
                >
                  Implemented flows · {detail.flows.length}
                </div>
                <div style={{ display: "flex", flexWrap: "wrap", gap: 4 }}>
                  {detail.flows.map((f) => (
                    <span
                      key={f}
                      style={{
                        fontSize: 10.5,
                        padding: "2px 6px",
                        borderRadius: 3,
                        background: T.successSoft,
                        color: T.success,
                        fontWeight: 600,
                      }}
                    >
                      {f}
                    </span>
                  ))}
                </div>
                {detail.probe && (
                  <div style={{ fontSize: 10.5, color: T.textSubtle, marginTop: 8, lineHeight: 1.5 }}>
                    Field probe (secondary, not scored): {detail.probe.buildableFlows} of{" "}
                    {detail.probe.probedFlows} probed flows could build a request. The probe
                    disagrees with source on ~130 cells fleet-wide, so it never scores anything.
                  </div>
                )}
              </div>


              {(["gap", "unknown", "met"] as const).map((state) => {
                const items = DATA.capabilities.filter((cap) => detail.cells[cap.id] === state);
                if (items.length === 0) return null;
                return (
                  <div key={state}>
                    <div
                      style={{
                        fontSize: 10.5,
                        fontWeight: 700,
                        textTransform: "uppercase",
                        letterSpacing: 0.5,
                        color: CELL_STYLE[state].fg,
                        marginBottom: 8,
                      }}
                    >
                      {state === "gap"
                        ? `To reach MVP · ${items.length}`
                        : state === "unknown"
                          ? `Unknown · ${items.length}`
                          : `Implemented · ${items.length}`}
                    </div>
                    <div style={{ display: "flex", flexDirection: "column", gap: 7 }}>
                      {items.map((cap) => (
                        <div
                          key={cap.id}
                          style={{
                            border: `1px solid ${T.border}`,
                            borderLeft: `3px solid ${CELL_STYLE[state].fg}`,
                            borderRadius: 6,
                            padding: "8px 11px",
                            background: T.bg,
                          }}
                        >
                          <div style={{ display: "flex", justifyContent: "space-between", gap: 8 }}>
                            <span style={{ fontSize: 12.5, fontWeight: 600 }}>{cap.label}</span>
                            {state === "gap" && (
                              <span style={{ fontSize: 11, color: T.textMuted, flexShrink: 0 }}>
                                {cap.weight} pts
                              </span>
                            )}
                          </div>
                          <div style={{ fontSize: 11, color: T.textMuted, marginTop: 3, lineHeight: 1.45 }}>
                            {cap.description}
                          </div>
                          {/* The signal is shown so "why is this a gap?" is answerable. */}
                          <code
                            style={{
                              display: "block",
                              marginTop: 5,
                              fontSize: 10,
                              color: T.textSubtle,
                              background: T.codeBg,
                              padding: "3px 6px",
                              borderRadius: 4,
                              wordBreak: "break-all",
                            }}
                          >
                            {JSON.stringify(cap.signal)}
                          </code>
                        </div>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          </aside>
        )}
      </div>
    </SidebarLayout>
  );
}

function RefreshButton({ loading, onClick }: { loading: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      disabled={loading}
      title="Re-derive from data/field_probe and the hyperswitch checkout"
      style={{
        padding: "8px 13px",
        borderRadius: 6,
        border: `1px solid ${T.border}`,
        background: T.bg,
        color: loading ? T.textSubtle : T.text,
        fontSize: 12.5,
        cursor: loading ? "default" : "pointer",
      }}
    >
      {loading ? "Refreshing…" : "↻ Refresh"}
    </button>
  );
}

const thBase: React.CSSProperties = {
  position: "sticky",
  top: 0,
  background: T.bgRightHeader,
  borderBottom: `1px solid ${T.borderStrong}`,
  padding: "8px 10px",
  fontSize: 11,
  fontWeight: 700,
  color: T.textMuted,
  verticalAlign: "bottom",
  textAlign: "center",
};

const tdBase: React.CSSProperties = {
  padding: "7px 10px",
  borderBottom: `1px solid ${T.border}`,
  verticalAlign: "middle",
};
