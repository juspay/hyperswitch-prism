import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { SidebarLayout } from "../components/NavigationSidebar";
import { ParityRunDrawer } from "../components/ParityRunDrawer";
import { T } from "../theme";
import { deriveStatus, statusColor, type LeafStatus, type ParityLeaf } from "../types/parity";

type StatusFilter = "all" | LeafStatus;

interface LockState {
  busy: boolean;
  leaf?: number;
  dryRun?: boolean;
  startedAt?: number;
}

interface ConfigStatus {
  prismPath: string;
  oracleReadOnlyPath: string;
  bridgeWritePath: string;
  hasOracle: boolean;
  hasBridge: boolean;
  runner: string;
  githubActor: string;
}

interface TreeResponse {
  leaves?: ParityLeaf[];
  error?: string;
}

export function ParityPage() {
  const [leaves, setLeaves] = useState<ParityLeaf[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [refreshedAt, setRefreshedAt] = useState<string>("");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [connectorFilter, setConnectorFilter] = useState<string>("all");
  const [search, setSearch] = useState<string>("");
  const [lock, setLock] = useState<LockState>({ busy: false });
  const [activeLeaf, setActiveLeaf] = useState<ParityLeaf | null>(null);
  const [config, setConfig] = useState<ConfigStatus | null>(null);
  const [configError, setConfigError] = useState<string | null>(null);

  async function pollLock() {
    try {
      const r = await fetch("/api/parity/lock");
      if (r.ok) setLock(await r.json());
    } catch { /* ignore transient */ }
  }
  useEffect(() => {
    pollLock();
    const t = setInterval(pollLock, 5000);
    return () => clearInterval(t);
  }, []);

  useEffect(() => {
    fetch("/api/parity/config")
      .then(async (r) => {
        const data = await r.json();
        if (!r.ok) {
          setConfigError(data.error ?? `HTTP ${r.status}`);
          return;
        }
        setConfig(data as ConfigStatus);
      })
      .catch((e) => setConfigError(String(e)));
  }, []);

  async function load() {
    setError(null);
    try {
      const res = await fetch("/api/parity/tree.json");
      const data = (await res.json()) as ParityLeaf[] | TreeResponse;
      if (!res.ok) {
        setError((data as TreeResponse).error ?? `HTTP ${res.status}`);
        return;
      }
      const arr = Array.isArray(data) ? data : [];
      setLeaves(arr);
      const mtime = res.headers.get("x-parity-mtime") ?? "";
      setRefreshedAt(mtime);
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    load();
  }, []);

  const connectors = useMemo(() => {
    if (!leaves) return [];
    return [...new Set(leaves.map((l) => l.connector))].sort();
  }, [leaves]);

  const filtered = useMemo(() => {
    if (!leaves) return [];
    const q = search.trim().toLowerCase();
    return leaves.filter((l) => {
      if (statusFilter !== "all" && deriveStatus(l) !== statusFilter) return false;
      if (connectorFilter !== "all" && l.connector !== connectorFilter) return false;
      if (q && !l.title.toLowerCase().includes(q) && !`#${l.number}`.includes(q)) return false;
      return true;
    });
  }, [leaves, statusFilter, connectorFilter, search]);

  const counts = useMemo(() => {
    if (!leaves) return { total: 0, "no-pr": 0, "pr-open": 0, "pr-merged": 0, blocked: 0 };
    const c: Record<string, number> = { total: leaves.length, "no-pr": 0, "pr-open": 0, "pr-merged": 0, blocked: 0 };
    for (const l of leaves) {
      const s = deriveStatus(l);
      c[s] = (c[s] ?? 0) + 1;
    }
    return c;
  }, [leaves]);

  return (
    <SidebarLayout>
      <div style={{ background: T.bg, minHeight: "100vh", padding: "32px 40px" }}>
        <header style={{ marginBottom: 24 }}>
          <div style={{ display: "flex", alignItems: "baseline", gap: 16, marginBottom: 6 }}>
            <h1 style={{ margin: 0, fontSize: 22, fontWeight: 700, color: T.text }}>Parity Checker</h1>
            <span style={{ fontSize: 12, color: T.textMuted }}>
              juspay/hyperswitch-cloud#15576 · prism ↔ hyperswitch parity
            </span>
          </div>
          <div style={{ fontSize: 12, color: T.textMuted }}>
            {refreshedAt ? `tree cache mtime: ${refreshedAt}` : "loading…"}{" "}
            <button
              onClick={load}
              style={{
                marginLeft: 8,
                background: T.accentSoft,
                border: `1px solid ${T.border}`,
                borderRadius: 6,
                padding: "3px 10px",
                fontSize: 11,
                cursor: "pointer",
                color: T.text,
              }}
            >
              ↻ refresh
            </button>{" "}
            <span style={{ marginLeft: 8, color: T.textSubtle }}>
              (run <code style={{ background: T.codeBg, padding: "1px 5px", borderRadius: 4 }}>10xgrace parity dashboard</code> to regenerate)
            </span>
          </div>
        </header>

        {error && (
          <div
            style={{
              background: T.errorSoft,
              color: T.error,
              padding: "12px 16px",
              borderRadius: 8,
              border: `1px solid ${T.error}`,
              marginBottom: 16,
            }}
          >
            {error}
          </div>
        )}

        {configError && (
          <div
            style={{
              background: T.warnSoft,
              color: T.warn,
              padding: "10px 14px",
              borderRadius: 8,
              border: `1px solid ${T.warn}`,
              marginBottom: 16,
              fontSize: 12,
            }}
          >
            <strong>Config load failed:</strong> {configError}
          </div>
        )}

        {config && (
          <section
            style={{
              display: "flex",
              flexWrap: "wrap",
              gap: 16,
              padding: "10px 14px",
              background: T.bgElev,
              border: `1px solid ${T.border}`,
              borderRadius: 6,
              marginBottom: 16,
              fontSize: 12,
              color: T.text,
            }}
          >
            <ConfigRow label="prism" value={config.prismPath} ok />
            <ConfigRow
              label="oracle"
              value={config.hasOracle ? config.oracleReadOnlyPath : "not configured (PARITY_ORACLE_PATH)"}
              ok={config.hasOracle}
            />
            <ConfigRow
              label="bridge"
              value={
                config.hasBridge
                  ? config.bridgeWritePath
                  : "not configured (PARITY_BRIDGE_PATH) — hs-bridge leaves will escalate"
              }
              ok={config.hasBridge}
            />
            <ConfigRow label="runner" value={config.runner} ok />
            {config.githubActor && <ConfigRow label="actor" value={`@${config.githubActor}`} ok />}
          </section>
        )}

        {!error && leaves && (
          <>
            {/* Summary chips */}
            <section style={{ display: "flex", gap: 12, marginBottom: 24, flexWrap: "wrap" }}>
              {(
                [
                  ["all", "All", counts.total],
                  ["no-pr", "No PR", counts["no-pr"]],
                  ["pr-open", "PR Open", counts["pr-open"]],
                  ["pr-merged", "Merged", counts["pr-merged"]],
                  ["blocked", "Blocked", counts.blocked],
                ] as const
              ).map(([key, label, count]) => {
                const sel = statusFilter === key;
                const c = key === "all" ? { fg: T.text, bg: T.bgElev } : statusColor(key as LeafStatus);
                return (
                  <button
                    key={key}
                    onClick={() => setStatusFilter(key as StatusFilter)}
                    style={{
                      border: `1px solid ${sel ? T.accent : T.border}`,
                      background: sel ? T.accentSoft : c.bg,
                      color: c.fg,
                      borderRadius: 8,
                      padding: "10px 14px",
                      cursor: "pointer",
                      fontSize: 13,
                      fontWeight: 600,
                      minWidth: 90,
                      textAlign: "left",
                    }}
                  >
                    <div style={{ fontSize: 18, fontWeight: 800 }}>{count}</div>
                    <div style={{ fontSize: 11, fontWeight: 500 }}>{label}</div>
                  </button>
                );
              })}
            </section>

            {/* Filters */}
            <section style={{ display: "flex", gap: 12, marginBottom: 16, alignItems: "center", flexWrap: "wrap" }}>
              <label style={{ fontSize: 12, color: T.textMuted }}>
                Connector:{" "}
                <select
                  value={connectorFilter}
                  onChange={(e) => setConnectorFilter(e.target.value)}
                  style={{
                    background: T.bgElev,
                    border: `1px solid ${T.border}`,
                    borderRadius: 6,
                    padding: "4px 8px",
                    fontSize: 12,
                    color: T.text,
                  }}
                >
                  <option value="all">All ({connectors.length})</option>
                  {connectors.map((c) => (
                    <option key={c} value={c}>
                      {c}
                    </option>
                  ))}
                </select>
              </label>
              <input
                placeholder="Search title or #N…"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                style={{
                  flex: "1 1 240px",
                  maxWidth: 360,
                  background: T.bgElev,
                  border: `1px solid ${T.border}`,
                  borderRadius: 6,
                  padding: "6px 10px",
                  fontSize: 12,
                  color: T.text,
                }}
              />
              <span style={{ fontSize: 12, color: T.textSubtle }}>
                showing {filtered.length} / {leaves.length}
              </span>
            </section>

            {/* Table */}
            <section
              style={{
                background: T.bgElev,
                border: `1px solid ${T.border}`,
                borderRadius: 10,
                overflow: "hidden",
              }}
            >
              <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13 }}>
                <thead>
                  <tr style={{ background: T.bgRightHeader }}>
                    <Th>Issue</Th>
                    <Th>Connector</Th>
                    <Th>Flow</Th>
                    <Th>Status</Th>
                    <Th>Linked PR</Th>
                    <Th>Created</Th>
                    <Th>Title</Th>
                    <Th>Action</Th>
                  </tr>
                </thead>
                <tbody>
                  {filtered
                    .slice()
                    .sort((a, b) => (a.createdAt < b.createdAt ? -1 : 1))
                    .map((l) => {
                      const s = deriveStatus(l);
                      const c = statusColor(s);
                      const pr = l.linkedPRs[0];
                      return (
                        <tr key={l.number} style={{ borderTop: `1px solid ${T.border}` }}>
                          <Td>
                            <a
                              href={l.url}
                              target="_blank"
                              rel="noreferrer"
                              style={{ color: T.accent, textDecoration: "none", fontWeight: 600 }}
                            >
                              #{l.number}
                            </a>
                          </Td>
                          <Td>
                            <Link
                              to={`/parity/${l.connector}`}
                              style={{ color: T.text, textDecoration: "none", fontWeight: 600 }}
                            >
                              {l.connector}
                            </Link>
                          </Td>
                          <Td><code style={{ background: T.codeBg, padding: "1px 6px", borderRadius: 4, fontSize: 11 }}>{l.flow}</code></Td>
                          <Td>
                            <span
                              style={{
                                background: c.bg,
                                color: c.fg,
                                padding: "2px 8px",
                                borderRadius: 4,
                                fontSize: 11,
                                fontWeight: 600,
                              }}
                            >
                              {s}
                            </span>
                          </Td>
                          <Td>
                            {pr ? (
                              <a href={pr.url} target="_blank" rel="noreferrer" style={{ color: T.accent, textDecoration: "none", fontSize: 12 }}>
                                {pr.repo}#{pr.number}
                              </a>
                            ) : (
                              <span style={{ color: T.textSubtle }}>—</span>
                            )}
                          </Td>
                          <Td style={{ color: T.textMuted, fontSize: 12, whiteSpace: "nowrap" }}>{l.createdAt.slice(0, 10)}</Td>
                          <Td style={{ color: T.text, fontSize: 12 }}>{l.title}</Td>
                          <Td>
                            {s === "no-pr" ? (
                              <button
                                onClick={() => setActiveLeaf(l)}
                                disabled={lock.busy && lock.leaf !== l.number}
                                title={
                                  lock.busy && lock.leaf !== l.number
                                    ? `another heartbeat is running (leaf #${lock.leaf})`
                                    : "Open autopilot drawer (defaults to dry-run)"
                                }
                                style={{
                                  background: lock.busy && lock.leaf !== l.number ? T.bgElev : T.accentSoft,
                                  color: lock.busy && lock.leaf !== l.number ? T.textSubtle : T.text,
                                  border: `1px solid ${T.border}`,
                                  borderRadius: 6,
                                  padding: "4px 10px",
                                  fontSize: 11,
                                  fontWeight: 600,
                                  cursor: lock.busy && lock.leaf !== l.number ? "not-allowed" : "pointer",
                                }}
                              >
                                ▶ Run
                              </button>
                            ) : (
                              <span style={{ color: T.textSubtle, fontSize: 11 }}>—</span>
                            )}
                          </Td>
                        </tr>
                      );
                    })}
                </tbody>
              </table>
              {filtered.length === 0 && (
                <div style={{ padding: 32, textAlign: "center", color: T.textMuted, fontSize: 13 }}>
                  No leaves match the current filters.
                </div>
              )}
            </section>
          </>
        )}

        {!error && !leaves && <div style={{ color: T.textMuted }}>Loading…</div>}

        {activeLeaf && (
          <ParityRunDrawer
            leaf={activeLeaf}
            onClose={() => setActiveLeaf(null)}
            onLockChange={() => pollLock()}
          />
        )}
      </div>
    </SidebarLayout>
  );
}

function ConfigRow({ label, value, ok }: { label: string; value: string; ok: boolean }) {
  return (
    <span style={{ display: "inline-flex", alignItems: "baseline", gap: 6 }}>
      <strong style={{ color: T.textMuted }}>{label}:</strong>{" "}
      {ok ? (
        <code style={{ background: T.codeBg, padding: "1px 6px", borderRadius: 4, fontSize: 11 }}>{value}</code>
      ) : (
        <span style={{ color: T.warn }}>{value}</span>
      )}
    </span>
  );
}

function Th({ children }: { children: React.ReactNode }) {
  return (
    <th
      style={{
        textAlign: "left",
        padding: "10px 14px",
        fontSize: 11,
        fontWeight: 700,
        color: T.textMuted,
        textTransform: "uppercase",
        letterSpacing: 0.4,
      }}
    >
      {children}
    </th>
  );
}

function Td({ children, style }: { children: React.ReactNode; style?: React.CSSProperties }) {
  return <td style={{ padding: "10px 14px", verticalAlign: "middle", ...style }}>{children}</td>;
}
