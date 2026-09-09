import { useMemo } from "react";
import { Link } from "react-router-dom";
import { SidebarLayout } from "../components/NavigationSidebar";
import { BarRow, Donut, Panel, StatCard } from "../components/mvp/primitives";
import { T } from "../theme";
import type { MvpConnector, MvpData } from "../types/mvp";
import { useMvpData } from "../lib/useMvpData";

const BUCKETS = [
  { label: "0–19%", lo: 0, hi: 19 },
  { label: "20–39%", lo: 20, hi: 39 },
  { label: "40–59%", lo: 40, hi: 59 },
  { label: "60–79%", lo: 60, hi: 79 },
  { label: "80–99%", lo: 80, hi: 99 },
  { label: "100%", lo: 100, hi: 100 },
];

function titleCase(name: string) {
  return name.replace(/_/g, " ").replace(/\b\w/g, (ch) => ch.toUpperCase());
}

/** Which UCS checkout produced these numbers, and any reason to distrust it. */
function ProvenanceNote({ data }: { data: MvpData }) {
  const g = data.provenance;
  if (!g) return null;
  const onMain = g.branch === "main" || g.branch === "master";
  const suspect = !onMain || g.dirty === true;
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 11, marginTop: 14, flexWrap: "wrap" }}>
      <span style={{ width: 132, color: T.textMuted, flexShrink: 0 }}>hyperswitch-prism</span>
      <code
        style={{
          background: suspect ? T.warnSoft : T.codeBg,
          color: suspect ? T.warn : T.textMuted,
          border: `1px solid ${suspect ? T.warn : "transparent"}`,
          padding: "1px 6px",
          borderRadius: 4,
        }}
      >
        {g.branch || "detached"} @ {g.commit}
        {g.dirty ? " · uncommitted changes" : ""}
        {!onMain ? " · not main" : ""}
      </code>
      <span style={{ color: T.textSubtle }}>UCS only — no hyperswitch data</span>
    </div>
  );
}

/** Leaderboard row. */
function LeaderRow({ c, max, color }: { c: MvpConnector; max: number; color: string }) {
  return (
    <div
      title={`${c.gaps} gaps · ${c.pct}% · ${c.flows.length} flows`}
      style={{ display: "flex", alignItems: "center", gap: 10, padding: "3px 0" }}
    >
      <div style={{ width: 186, display: "flex", alignItems: "center", justifyContent: "flex-end", gap: 6, flexShrink: 0, overflow: "hidden" }}>
        <span style={{ fontSize: 11.5, color: T.text, whiteSpace: "nowrap" }}>{titleCase(c.name)}</span>
      </div>
      <div style={{ flex: 1, height: 15, background: T.bgRight, borderRadius: 4, overflow: "hidden" }}>
        <div style={{ width: `${max > 0 ? (c.effort / max) * 100 : 0}%`, height: "100%", background: color, borderRadius: 4 }} />
      </div>
      <div style={{ width: 62, fontSize: 11.5, fontWeight: 600, color: T.textMuted, flexShrink: 0 }}>{c.effort} pts</div>
    </div>
  );
}

export function MvpMetricsPage() {
  const { data: DATA, refreshedAt, error, loading, refresh } = useMvpData();
  const { capabilities, connectors } = DATA;

  const m = useMemo(() => {
    const total = connectors.length;
    const atMvp = connectors.filter((c) => c.gaps === 0).length;
    const gaps = connectors.reduce((s, c) => s + c.gaps, 0);
    const effort = connectors.reduce((s, c) => s + c.effort, 0);
    const avg = total ? Math.round(connectors.reduce((s, c) => s + c.pct, 0) / total) : 0;

    const perCap = capabilities
      .map((cap) => {
        const k = { met: 0, gap: 0, unknown: 0, alias: 0 };
        for (const c of connectors) k[c.cells[cap.id]]++;
        return { cap, ...k };
      })
      .sort((a, b) => Number(a.alias > 0) - Number(b.alias > 0) || b.met - a.met);

    const histogram = BUCKETS.map((b) => ({
      ...b,
      count: connectors.filter((c) => c.pct >= b.lo && c.pct <= b.hi).length,
    }));

    const quickWins = connectors.filter((c) => c.gaps > 0).slice().sort((a, b) => a.effort - b.effort).slice(0, 12);
    const heaviest = connectors.slice().sort((a, b) => b.effort - a.effort).slice(0, 12);

    // Flow implementation counts, straight from the Rust source extraction.
    const flowCount = new Map<string, number>();
    for (const c of connectors) for (const f of c.flows) flowCount.set(f, (flowCount.get(f) ?? 0) + 1);
    const topFlows = [...flowCount.entries()].sort((a, b) => b[1] - a[1]).slice(0, 16);

    const unknownCells = connectors.reduce(
      (s, c) => s + capabilities.filter((cap) => c.cells[cap.id] === "unknown").length, 0);

    return { total, atMvp, gaps, effort, avg, perCap, histogram, quickWins, heaviest, topFlows, unknownCells };
  }, [capabilities, connectors]);

  const maxBucket = Math.max(...m.histogram.map((h) => h.count), 1);
  const scoredFields = capabilities.filter((c) => c.confidence === "proven").length;

  return (
    <SidebarLayout>
      <div style={{ minHeight: "100vh", background: T.bg, color: T.text, fontFamily: "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" }}>
        <style>{`
          html, body, #root { margin: 0; padding: 0; background: ${T.bg}; }
          * { box-sizing: border-box; }
        `}</style>

        <header style={{ padding: "20px 32px", borderBottom: `1px solid ${T.border}`, background: T.bgElev, display: "flex", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap", gap: 16 }}>
          <div>
            <h1 style={{ margin: 0, fontSize: 18, fontWeight: 700 }}>MVP Metrics</h1>
            <span style={{ fontSize: 12, color: T.textMuted }}>
              {m.total} connectors · scored on {scoredFields} proven capabilities from UCS Rust source
            </span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <button
              onClick={refresh}
              disabled={loading}
              style={{ padding: "8px 13px", borderRadius: 6, border: `1px solid ${T.border}`, background: T.bg, color: loading ? T.textSubtle : T.text, fontSize: 12.5, cursor: loading ? "default" : "pointer" }}
            >
              {loading ? "Refreshing…" : "↻ Refresh"}
            </button>
            <Link to="/mvp" style={{ fontSize: 12.5, fontWeight: 600, color: T.accent, textDecoration: "none", border: `1px solid ${T.border}`, borderRadius: 6, padding: "8px 14px", background: T.bg }}>
              ▦ Readiness matrix
            </Link>
          </div>
        </header>

        <div style={{ padding: "20px 32px 40px", display: "flex", flexDirection: "column", gap: 18, minWidth: 0 }}>
          <div style={{ display: "flex", gap: 14, flexWrap: "wrap" }}>
            <StatCard label="At MVP" value={`${m.atMvp} / ${m.total}`} tone={m.atMvp === 0 ? "bad" : "good"} hint="zero scored gaps" />
            <StatCard label="Average completeness" value={`${m.avg}%`} />
            <StatCard label="Capability gaps" value={m.gaps} tone="warn" hint="cells to close" />
            <StatCard label="Effort remaining" value={`${m.effort} pts`} hint="weighted by rubric" />
            <StatCard label="Rubric" value={`${scoredFields} / ${capabilities.length}`} hint="proven (scored) / total shown" />
          </div>


          <div style={{ display: "grid", gridTemplateColumns: "minmax(240px, 1fr) minmax(340px, 2fr)", gap: 18 }}>
            <Panel title="Fleet completeness" subtitle="mean across all connectors">
              <div style={{ display: "flex", justifyContent: "center", padding: "6px 0 2px" }}>
                <Donut pct={m.avg} caption={`${m.atMvp} of ${m.total} connectors at full MVP`} />
              </div>
            </Panel>
            <Panel title="Distribution" subtitle="share of connectors per completeness band">
              <div style={{ display: "flex", alignItems: "flex-end", gap: 10, height: 168, paddingTop: 8 }}>
                {m.histogram.map((h) => (
                  <div key={h.label} style={{ flex: 1, display: "flex", flexDirection: "column", alignItems: "center", gap: 6 }}>
                    <span style={{ fontSize: 12, fontWeight: 700 }}>
                      {m.total ? Math.round((h.count / m.total) * 100) : 0}%
                    </span>
                    <div
                      title={`${h.count} of ${m.total} connectors at ${h.label}`}
                      style={{
                        width: "100%",
                        height: `${(h.count / maxBucket) * 118}px`,
                        minHeight: h.count > 0 ? 4 : 1,
                        background: h.lo >= 80 ? T.success : h.lo >= 40 ? T.warn : T.accent,
                        borderRadius: "4px 4px 0 0",
                        opacity: h.count ? 1 : 0.25,
                      }}
                    />
                    <span style={{ fontSize: 10.5, color: T.textMuted }}>{h.label}</span>
                  </div>
                ))}
              </div>
            </Panel>
          </div>

          <Panel title="Capability adoption" subtitle="how many of the 108 connectors implement each rubric item">
            <div style={{ display: "flex", flexDirection: "column", gap: 1 }}>
              {m.perCap.map(({ cap, met, gap, unknown, alias }) => {
                if (alias > 0) {
                  return (
                    <div key={cap.id} title={cap.description} style={{ display: "flex", alignItems: "center", gap: 10, padding: "3px 0" }}>
                      <div style={{ width: 186, fontSize: 11.5, color: T.textSubtle, textAlign: "right", flexShrink: 0 }}>{cap.label}</div>
                      <div style={{ flex: 1, height: 15, borderRadius: 4, border: `1px dashed ${T.borderStrong}`, display: "flex", alignItems: "center", paddingLeft: 8, fontSize: 10, color: T.textSubtle, fontStyle: "italic" }}>
                        alias — duplicates another field, not scored
                      </div>
                      <div style={{ width: 62, fontSize: 11.5, color: T.textSubtle, flexShrink: 0 }}>—</div>
                    </div>
                  );
                }
                return (
                  <BarRow
                    key={cap.id}
                    label={cap.label}
                    value={met}
                    max={m.total}
                    suffix={unknown > 0 ? ` / ${m.total} · ${unknown}?` : ` / ${m.total}`}
                    color={met > m.total * 0.6 ? T.success : met > m.total * 0.2 ? T.warn : T.accent}
                    title={`${cap.label}\n${cap.description}\n\nimplemented ${met} · not implemented ${gap} · unknown ${unknown}`}
                  />
                );
              })}
            </div>
            {m.unknownCells > 0 && (
              <p style={{ fontSize: 11.5, color: T.textMuted, lineHeight: 1.6, marginTop: 14, marginBottom: 0 }}>
                <strong>{m.unknownCells} cells are UNKNOWN</strong> — the UCS source genuinely does not
                say. Mostly 3DS, where a connector emits a generic processor redirect URL and nothing in
                the repo reveals whether 3DS runs behind it. Shown as unknown rather than guessed, and
                excluded from every denominator.
              </p>
            )}
          </Panel>

          <Panel title="Flow implementation" subtitle="gRPC flows per connector, extracted from Rust source — the evidence behind the rubric">
            <div style={{ display: "flex", flexDirection: "column", gap: 1 }}>
              {m.topFlows.map(([flow, n]) => (
                <BarRow key={flow} label={flow} value={n} max={m.total} suffix={` / ${m.total}`}
                        color={n > m.total * 0.6 ? T.success : n > m.total * 0.2 ? T.warn : T.accent} />
              ))}
            </div>
          </Panel>

          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(340px, 1fr))", gap: 18 }}>
            <Panel title="Quick wins" subtitle="least effort to reach MVP">
              <div style={{ display: "flex", flexDirection: "column", gap: 1 }}>
                {m.quickWins.map((c) => (
                  <LeaderRow key={c.name} c={c} max={m.quickWins.at(-1)?.effort ?? 1} color={T.success} />
                ))}
              </div>
            </Panel>
            <Panel title="Heaviest lifts" subtitle="most effort to reach MVP">
              <div style={{ display: "flex", flexDirection: "column", gap: 1 }}>
                {m.heaviest.map((c) => (
                  <LeaderRow key={c.name} c={c} max={m.heaviest[0]?.effort ?? 1} color={T.accent} />
                ))}
              </div>
            </Panel>
          </div>

          <Panel title="Where the numbers come from" subtitle="UCS source only — no hyperswitch data">
            <p style={{ fontSize: 11.5, color: T.textMuted, lineHeight: 1.7, margin: 0 }}>
              Every scored cell is derived from the UCS Rust source: implemented gRPC flows extracted
              from <code style={{ background: T.codeBg, padding: "1px 5px", borderRadius: 4 }}>create_all_prerequisites!</code>,{" "}
              <code style={{ background: T.codeBg, padding: "1px 5px", borderRadius: 4 }}>macro_connector_implementation!</code>{" "}
              and hand-written impls, minus everything declared under{" "}
              <code style={{ background: T.codeBg, padding: "1px 5px", borderRadius: 4 }}>not_implemented</code>/
              <code style={{ background: T.codeBg, padding: "1px 5px", borderRadius: 4 }}>not_supported</code>.
              Trait-based capabilities check the override body, because overriding is not implementing —
              two dispute-webhook overrides return <code style={{ background: T.codeBg, padding: "1px 5px", borderRadius: 4 }}>WebhooksNotImplemented</code>.{" "}
              <strong>The field probe scores nothing</strong>: it records whether a request could be
              built, disagrees with source on ~130 cells, and four connectors with permissive
              transformers falsely report supporting 103 payment methods.
            </p>
            <ProvenanceNote data={DATA} />
          </Panel>

          <div style={{ fontSize: 11, color: T.textSubtle }}>
            Derived {new Date(refreshedAt || DATA.generatedAt).toLocaleString()} · regenerated from source on load
            {error ? " · last refresh failed, showing previous data" : ""}
          </div>
        </div>
      </div>
    </SidebarLayout>
  );
}
