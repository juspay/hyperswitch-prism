import { useEffect, useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { SidebarLayout } from "../components/NavigationSidebar";
import { T } from "../theme";
import { deriveStatus, statusColor, type ParityLeaf } from "../types/parity";

export function ParityConnectorPage() {
  const { connectorName } = useParams<{ connectorName: string }>();
  const [leaves, setLeaves] = useState<ParityLeaf[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetch("/api/parity/tree.json")
      .then(async (r) => {
        const data = await r.json();
        if (!r.ok) throw new Error(data.error ?? `HTTP ${r.status}`);
        return data as ParityLeaf[];
      })
      .then((arr) => setLeaves(arr.filter((l) => l.connector === connectorName)))
      .catch((e) => setError(String(e)));
  }, [connectorName]);

  const tracking = useMemo(() => {
    if (!leaves) return [];
    return [...new Set(leaves.map((l) => l.parentTracking))];
  }, [leaves]);

  return (
    <SidebarLayout>
      <div style={{ background: T.bg, minHeight: "100vh", padding: "32px 40px" }}>
        <div style={{ marginBottom: 16, fontSize: 12 }}>
          <Link to="/parity" style={{ color: T.accent, textDecoration: "none" }}>
            ← Parity Checker
          </Link>
        </div>
        <h1 style={{ margin: "0 0 6px 0", fontSize: 22, color: T.text }}>
          {connectorName}
        </h1>
        <div style={{ fontSize: 12, color: T.textMuted, marginBottom: 24 }}>
          tracking {tracking.map((n) => `#${n}`).join(" ") || "—"}
        </div>

        {error && (
          <div style={{ background: T.errorSoft, color: T.error, padding: 12, borderRadius: 8 }}>{error}</div>
        )}

        {leaves && (
          <section style={{ background: T.bgElev, border: `1px solid ${T.border}`, borderRadius: 10, overflow: "hidden" }}>
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13 }}>
              <thead>
                <tr style={{ background: T.bgRightHeader }}>
                  <th style={th}>Issue</th>
                  <th style={th}>Flow</th>
                  <th style={th}>Status</th>
                  <th style={th}>Linked PRs</th>
                  <th style={th}>Created</th>
                  <th style={th}>Title</th>
                </tr>
              </thead>
              <tbody>
                {leaves
                  .slice()
                  .sort((a, b) => a.number - b.number)
                  .map((l) => {
                    const s = deriveStatus(l);
                    const c = statusColor(s);
                    return (
                      <tr key={l.number} style={{ borderTop: `1px solid ${T.border}` }}>
                        <td style={td}>
                          <a href={l.url} target="_blank" rel="noreferrer" style={{ color: T.accent, textDecoration: "none", fontWeight: 600 }}>
                            #{l.number}
                          </a>
                        </td>
                        <td style={td}>
                          <code style={{ background: T.codeBg, padding: "1px 6px", borderRadius: 4, fontSize: 11 }}>{l.flow}</code>
                        </td>
                        <td style={td}>
                          <span style={{ background: c.bg, color: c.fg, padding: "2px 8px", borderRadius: 4, fontSize: 11, fontWeight: 600 }}>
                            {s}
                          </span>
                        </td>
                        <td style={td}>
                          {l.linkedPRs.length === 0
                            ? <span style={{ color: T.textSubtle }}>—</span>
                            : (
                              <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
                                {l.linkedPRs.map((pr) => (
                                  <a key={`${pr.repo}#${pr.number}`} href={pr.url} target="_blank" rel="noreferrer"
                                     style={{ color: T.accent, textDecoration: "none", fontSize: 12 }}>
                                    {pr.repo}#{pr.number} ({pr.state})
                                  </a>
                                ))}
                              </div>
                            )}
                        </td>
                        <td style={{ ...td, color: T.textMuted, fontSize: 12, whiteSpace: "nowrap" }}>{l.createdAt.slice(0, 10)}</td>
                        <td style={{ ...td, color: T.text, fontSize: 12 }}>{l.title}</td>
                      </tr>
                    );
                  })}
              </tbody>
            </table>
          </section>
        )}
      </div>
    </SidebarLayout>
  );
}

const th: React.CSSProperties = {
  textAlign: "left",
  padding: "10px 14px",
  fontSize: 11,
  fontWeight: 700,
  color: T.textMuted,
  textTransform: "uppercase",
  letterSpacing: 0.4,
};
const td: React.CSSProperties = { padding: "10px 14px", verticalAlign: "middle" };
