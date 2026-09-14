import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { ParityRunDrawer } from "../components/ParityRunDrawer";
import { T } from "../theme";
import { deriveStatus, statusColor, type LeafStatus, type ParityLeaf } from "../types/parity";

/**
 * GRACE-style 4-pane Parity Checker:
 * - Left:   ParitySidebar (status-grouped leaf rows, status indicators)
 * - Centre: TopBar (config/counts) + LeafDetail (selected leaf body / linked PRs / actions)
 * - Right:  ParityLogPanel (recent action logs + lock state)
 * - Bottom: ParityJourneyBar (compact status counters)
 *
 * Mirrors WorkflowPage's layout for visual consistency. The supervisor
 * (packages/cli/src/commands/supervisor.ts) keeps the tree cache warm
 * via a 5-min background refresh; the dashboard polls /api/parity/lock
 * every 5s and reloads the tree when it sees a fresh mtime.
 */

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

const STATUS_GROUPS: Array<{ key: LeafStatus; label: string }> = [
  { key: "no-pr", label: "No PR" },
  { key: "pr-open", label: "PR Open" },
  { key: "pr-merged", label: "Merged" },
  { key: "blocked", label: "Blocked" },
  { key: "closed", label: "Closed" },
  { key: "not-applicable", label: "N/A" },
];

export function ParityPage() {
  const [leaves, setLeaves] = useState<ParityLeaf[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [refreshedAt, setRefreshedAt] = useState<string>("");
  const [lock, setLock] = useState<LockState>({ busy: false });
  const [config, setConfig] = useState<ConfigStatus | null>(null);
  const [configError, setConfigError] = useState<string | null>(null);
  const [selectedNumber, setSelectedNumber] = useState<number | null>(null);
  const [search, setSearch] = useState("");
  const [activeLeaf, setActiveLeaf] = useState<ParityLeaf | null>(null);

  // Poll lock state — shows which leaf (if any) the parity heartbeat is
  // currently working on. Also used by the supervisor's background refresh
  // mtime indirectly via the tree fetch.
  async function pollLock() {
    try {
      const r = await fetch("/api/parity/lock");
      if (r.ok) setLock(await r.json());
    } catch {
      /* ignore transient */
    }
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

  async function load(opts: { force?: boolean } = {}) {
    setError(null);
    try {
      const qs = opts.force ? "?force=1" : "";
      const res = await fetch(`/api/parity/tree.json${qs}`);
      const data = (await res.json()) as ParityLeaf[] | TreeResponse;
      if (!res.ok) {
        setError((data as TreeResponse).error ?? `HTTP ${res.status}`);
        return;
      }
      const arr = Array.isArray(data) ? data : [];
      setLeaves(arr);
      setRefreshedAt(res.headers.get("x-parity-mtime") ?? "");
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    load();
    // Poll for fresh data every 60s — the supervisor refreshes the cache
    // every 5min, but a tighter dashboard poll picks up the update faster.
    const t = setInterval(() => void load(), 60_000);
    return () => clearInterval(t);
  }, []);

  // Auto-select the first leaf when leaves load so the centre pane isn't blank.
  // Also reconcile when the currently-selected leaf has disappeared server-side
  // (e.g. the issue was closed and pruned from the tree on a refresh) — flip
  // to the new first leaf instead of leaving the detail pane silently empty.
  useEffect(() => {
    if (!leaves || leaves.length === 0) return;
    if (selectedNumber === null) {
      setSelectedNumber(leaves[0].number);
      return;
    }
    const stillPresent = leaves.some((l) => l.number === selectedNumber);
    if (!stillPresent) {
      setSelectedNumber(leaves[0].number);
    }
  }, [leaves, selectedNumber]);

  const filteredLeaves = useMemo(() => {
    if (!leaves) return null;
    const q = search.trim().toLowerCase();
    if (!q) return leaves;
    return leaves.filter(
      (l) =>
        l.title.toLowerCase().includes(q) ||
        l.connector.toLowerCase().includes(q) ||
        l.flow.toLowerCase().includes(q) ||
        `#${l.number}`.includes(q),
    );
  }, [leaves, search]);

  const counts = useMemo(() => {
    if (!leaves) return { total: 0, "no-pr": 0, "pr-open": 0, "pr-merged": 0, blocked: 0 };
    const c: Record<string, number> = { total: leaves.length, "no-pr": 0, "pr-open": 0, "pr-merged": 0, blocked: 0 };
    for (const l of leaves) {
      const s = deriveStatus(l);
      c[s] = (c[s] ?? 0) + 1;
    }
    return c;
  }, [leaves]);

  const selectedLeaf = selectedNumber != null
    ? leaves?.find((l) => l.number === selectedNumber) ?? null
    : null;

  return (
    <div
      style={{
        display: "flex",
        height: "100vh",
        background: T.bg,
        color: T.text,
        fontFamily:
          "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
      }}
    >
      <style>{`
        @keyframes spin { to { transform: rotate(360deg) } }
        @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.55} }
        html, body, #root { margin: 0; padding: 0; height: 100%; background: ${T.bg}; }
        * { box-sizing: border-box; }
        ::-webkit-scrollbar { width: 10px; height: 10px; }
        ::-webkit-scrollbar-track { background: ${T.bg}; }
        ::-webkit-scrollbar-thumb { background: ${T.border}; border-radius: 5px; }
        ::-webkit-scrollbar-thumb:hover { background: ${T.borderStrong}; }
      `}</style>

      <ParitySidebar
        leaves={filteredLeaves}
        selectedNumber={selectedNumber}
        onSelect={setSelectedNumber}
        refreshedAt={refreshedAt}
        onRefresh={() => load({ force: true })}
        search={search}
        onSearch={setSearch}
      />

      <main
        style={{
          flex: 1,
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
          minHeight: 0,
        }}
      >
        <ParityTopBar
          counts={counts}
          config={config}
          configError={configError}
          error={error}
        />

        <div
          style={{
            flex: 1,
            minHeight: 0,
            overflow: "hidden",
            display: "flex",
            flexDirection: "column",
          }}
        >
          {selectedLeaf ? (
            <ParityLeafDetail
              leaf={selectedLeaf}
              lock={lock}
              onRun={() => setActiveLeaf(selectedLeaf)}
            />
          ) : (
            <ParityEmptyState loading={!leaves && !error} />
          )}
        </div>

        <ParityJourneyBar counts={counts} />
      </main>

      <ParityLogPanel lock={lock} refreshedAt={refreshedAt} />

      {activeLeaf && (
        <ParityRunDrawer
          leaf={activeLeaf}
          onClose={() => setActiveLeaf(null)}
          onLockChange={() => pollLock()}
        />
      )}
    </div>
  );
}

// ─── Sidebar (left pane) ──────────────────────────────────────────────────

function ParitySidebar({
  leaves,
  selectedNumber,
  onSelect,
  refreshedAt,
  onRefresh,
  search,
  onSearch,
}: {
  leaves: ParityLeaf[] | null;
  selectedNumber: number | null;
  onSelect: (n: number) => void;
  refreshedAt: string;
  onRefresh: () => void;
  search: string;
  onSearch: (s: string) => void;
}) {
  const grouped = useMemo(() => {
    const out: Record<LeafStatus, ParityLeaf[]> = {
      "no-pr": [],
      "pr-open": [],
      "pr-merged": [],
      blocked: [],
      closed: [],
      "not-applicable": [],
    };
    for (const l of leaves ?? []) {
      const s = deriveStatus(l);
      out[s].push(l);
    }
    // Stable sort within each group by number for predictability.
    for (const key of Object.keys(out) as LeafStatus[]) {
      out[key].sort((a, b) => a.number - b.number);
    }
    return out;
  }, [leaves]);

  const navigate = useNavigate();

  return (
    <aside
      style={{
        width: 360,
        background: T.bgSidebar,
        borderRight: `1px solid ${T.border}`,
        display: "flex",
        flexDirection: "column",
        height: "100vh",
      }}
    >
      <div
        style={{
          padding: "18px 20px 14px",
          borderBottom: `1px solid ${T.border}`,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <div
            style={{
              width: 30,
              height: 30,
              borderRadius: 8,
              background: T.accent,
              color: "#fff",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontSize: 14,
              fontWeight: 800,
            }}
          >
            ⚖
          </div>
          <div style={{ minWidth: 0 }}>
            <div style={{ fontSize: 15, fontWeight: 700, color: T.text, lineHeight: 1.2 }}>
              Parity Checker
            </div>
            <div style={{ fontSize: 10, color: T.textMuted, marginTop: 2 }}>
              prism ↔ hyperswitch
            </div>
          </div>
        </div>

        <button
          onClick={() => navigate("/")}
          style={{
            marginTop: 12,
            background: "transparent",
            color: T.textMuted,
            border: "none",
            padding: 0,
            fontSize: 11,
            cursor: "pointer",
          }}
        >
          ← Back to home
        </button>

        <div
          style={{
            marginTop: 14,
            display: "flex",
            alignItems: "center",
            gap: 8,
            fontSize: 11,
          }}
        >
          <span
            style={{
              flex: 1,
              color: T.textSubtle,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
            title={refreshedAt}
          >
            {refreshedAt ? `mtime: ${refreshedAt.slice(11, 19)}` : "loading…"}
          </span>
          <button
            onClick={onRefresh}
            title="Force a fresh GitHub fetch (bypass 5-min cache)"
            style={{
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
          </button>
        </div>

        <input
          placeholder="Filter by connector, flow, #N…"
          value={search}
          onChange={(e) => onSearch(e.target.value)}
          style={{
            marginTop: 12,
            width: "100%",
            background: T.bgElev,
            border: `1px solid ${T.border}`,
            borderRadius: 6,
            padding: "6px 10px",
            fontSize: 12,
            color: T.text,
            outline: "none",
          }}
        />
      </div>

      <div style={{ flex: 1, overflowY: "auto", padding: "10px 0 24px" }}>
        {STATUS_GROUPS.map(({ key, label }) => {
          const rows = grouped[key];
          if (rows.length === 0) return null;
          return (
            <ParitySidebarGroup
              key={key}
              label={label}
              status={key}
              rows={rows}
              selectedNumber={selectedNumber}
              onSelect={onSelect}
            />
          );
        })}
        {leaves && leaves.length === 0 && (
          <div style={{ padding: "16px 22px", fontSize: 12, color: T.textMuted }}>
            No leaves match the filter.
          </div>
        )}
      </div>
    </aside>
  );
}

function ParitySidebarGroup({
  label,
  status,
  rows,
  selectedNumber,
  onSelect,
}: {
  label: string;
  status: LeafStatus;
  rows: ParityLeaf[];
  selectedNumber: number | null;
  onSelect: (n: number) => void;
}) {
  const c = statusColor(status);
  return (
    <div style={{ marginBottom: 6 }}>
      <div
        style={{
          padding: "10px 24px 6px",
          fontSize: 10,
          fontWeight: 700,
          color: T.textSubtle,
          textTransform: "uppercase",
          letterSpacing: 1.2,
          display: "flex",
          alignItems: "center",
          gap: 8,
        }}
      >
        <span>{label}</span>
        <span
          style={{
            background: c.bg,
            color: c.fg,
            padding: "1px 7px",
            borderRadius: 999,
            fontSize: 10,
            fontWeight: 700,
          }}
        >
          {rows.length}
        </span>
        <span style={{ flex: 1, height: 1, background: T.border }} />
      </div>
      <div style={{ padding: "0 10px" }}>
        {rows.map((l) => (
          <ParityLeafRow
            key={l.number}
            leaf={l}
            isSelected={selectedNumber === l.number}
            onSelect={() => onSelect(l.number)}
          />
        ))}
      </div>
    </div>
  );
}

function ParityLeafRow({
  leaf,
  isSelected,
  onSelect,
}: {
  leaf: ParityLeaf;
  isSelected: boolean;
  onSelect: () => void;
}) {
  const status = deriveStatus(leaf);
  const c = statusColor(status);
  return (
    <button
      onClick={onSelect}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        width: "100%",
        padding: "8px 12px",
        margin: "2px 0",
        borderRadius: 8,
        background: isSelected ? T.accentSoft : "transparent",
        border: isSelected ? `1px solid ${T.accent}` : "1px solid transparent",
        cursor: "pointer",
        textAlign: "left",
        transition: "background 120ms, border-color 120ms",
      }}
      onMouseEnter={(e) => {
        if (!isSelected) e.currentTarget.style.background = "#f5ead0";
      }}
      onMouseLeave={(e) => {
        if (!isSelected) e.currentTarget.style.background = "transparent";
      }}
    >
      <span
        style={{
          width: 8,
          height: 8,
          borderRadius: "50%",
          background: c.fg,
          flexShrink: 0,
          boxShadow: `0 0 0 3px ${c.bg}`,
        }}
      />
      <div style={{ minWidth: 0, flex: 1 }}>
        <div
          style={{
            fontSize: 12.5,
            fontWeight: 600,
            color: T.text,
            display: "flex",
            alignItems: "center",
            gap: 6,
            overflow: "hidden",
          }}
        >
          <span
            style={{
              fontSize: 10,
              color: T.textSubtle,
              fontFamily: "ui-monospace, monospace",
              fontWeight: 600,
              flexShrink: 0,
            }}
          >
            #{leaf.number}
          </span>
          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {leaf.connector}
          </span>
        </div>
        <div
          style={{
            fontSize: 10.5,
            color: T.textMuted,
            marginTop: 2,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {leaf.flow}
        </div>
      </div>
    </button>
  );
}

// ─── Top bar (counts + config) ────────────────────────────────────────────

function ParityTopBar({
  counts,
  config,
  configError,
  error,
}: {
  counts: Record<string, number>;
  config: ConfigStatus | null;
  configError: string | null;
  error: string | null;
}) {
  return (
    <header
      style={{
        borderBottom: `1px solid ${T.border}`,
        background: T.bgElev,
        padding: "14px 28px",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          flexWrap: "wrap",
          gap: 16,
          fontSize: 12,
        }}
      >
        <div style={{ display: "flex", gap: 8 }}>
          {(
            [
              ["all", "Total", counts.total],
              ["no-pr", "No PR", counts["no-pr"]],
              ["pr-open", "PR Open", counts["pr-open"]],
              ["pr-merged", "Merged", counts["pr-merged"]],
              ["blocked", "Blocked", counts.blocked],
            ] as const
          ).map(([key, label, count]) => {
            const c =
              key === "all"
                ? { fg: T.text, bg: T.bgRightHeader }
                : statusColor(key as LeafStatus);
            return (
              <div
                key={key}
                style={{
                  background: c.bg,
                  color: c.fg,
                  borderRadius: 6,
                  padding: "6px 10px",
                  display: "flex",
                  alignItems: "baseline",
                  gap: 6,
                  border: `1px solid ${T.border}`,
                }}
              >
                <span style={{ fontSize: 15, fontWeight: 800 }}>{count}</span>
                <span style={{ fontSize: 10, fontWeight: 600 }}>{label}</span>
              </div>
            );
          })}
        </div>

        <span style={{ flex: 1 }} />

        <div style={{ display: "flex", flexWrap: "wrap", gap: 14, fontSize: 11, color: T.textMuted }}>
          {config && (
            <>
              <ConfigChip label="prism" value={config.prismPath} ok />
              <ConfigChip
                label="oracle"
                value={config.hasOracle ? config.oracleReadOnlyPath : "not configured"}
                ok={config.hasOracle}
              />
              <ConfigChip
                label="bridge"
                value={config.hasBridge ? config.bridgeWritePath : "not configured"}
                ok={config.hasBridge}
              />
              <ConfigChip label="runner" value={config.runner} ok />
            </>
          )}
          {configError && (
            <span style={{ color: T.warn }}>config error: {configError}</span>
          )}
        </div>
      </div>

      {error && (
        <div
          style={{
            marginTop: 10,
            background: T.errorSoft,
            color: T.error,
            padding: "8px 12px",
            borderRadius: 6,
            border: `1px solid ${T.error}`,
            fontSize: 12,
          }}
        >
          {error}
        </div>
      )}
    </header>
  );
}

function ConfigChip({
  label,
  value,
  ok,
}: {
  label: string;
  value: string;
  ok: boolean;
}) {
  return (
    <span style={{ display: "inline-flex", alignItems: "baseline", gap: 5 }}>
      <strong style={{ color: T.textMuted, fontWeight: 700 }}>{label}:</strong>
      {ok ? (
        <code
          style={{
            background: T.codeBg,
            padding: "1px 6px",
            borderRadius: 4,
            fontSize: 10.5,
            color: T.text,
          }}
        >
          {value.length > 36 ? "…" + value.slice(-34) : value}
        </code>
      ) : (
        <span style={{ color: T.warn }}>{value}</span>
      )}
    </span>
  );
}

// ─── Detail pane (centre) ─────────────────────────────────────────────────

function ParityLeafDetail({
  leaf,
  lock,
  onRun,
}: {
  leaf: ParityLeaf;
  lock: LockState;
  onRun: () => void;
}) {
  const status = deriveStatus(leaf);
  const c = statusColor(status);
  const isLockedByOther = lock.busy && lock.leaf !== leaf.number;
  const canRun = status === "no-pr";

  return (
    <div
      style={{
        padding: "28px 36px 36px",
        flex: 1,
        overflowY: "auto",
        minHeight: 0,
      }}
    >
      <div
        style={{
          fontSize: 11,
          color: T.textSubtle,
          fontWeight: 600,
          textTransform: "uppercase",
          letterSpacing: 1.2,
          marginBottom: 8,
        }}
      >
        Leaf · {leaf.connector} / {leaf.flow}
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 14, marginBottom: 18, flexWrap: "wrap" }}>
        <a
          href={leaf.url}
          target="_blank"
          rel="noreferrer"
          style={{ color: T.text, textDecoration: "none" }}
        >
          <h1 style={{ margin: 0, fontSize: 22, fontWeight: 700, letterSpacing: -0.2 }}>
            #{leaf.number}{" "}
            <span style={{ color: T.accent, fontSize: 14, fontWeight: 600 }}>↗</span>
          </h1>
        </a>
        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 7,
            background: c.bg,
            color: c.fg,
            padding: "5px 12px 5px 10px",
            borderRadius: 999,
            fontSize: 11,
            fontWeight: 700,
            letterSpacing: 0.3,
          }}
        >
          <span style={{ width: 7, height: 7, borderRadius: "50%", background: c.fg }} />
          {status}
        </span>
        {canRun && (
          <button
            onClick={onRun}
            disabled={isLockedByOther}
            title={
              isLockedByOther
                ? `another heartbeat is running (leaf #${lock.leaf})`
                : "Open autopilot drawer (defaults to dry-run)"
            }
            style={{
              marginLeft: "auto",
              fontSize: 12,
              fontWeight: 700,
              padding: "6px 14px",
              borderRadius: 6,
              border: `1px solid ${isLockedByOther ? T.border : T.accent}`,
              background: isLockedByOther ? T.bgElev : T.accentSoft,
              color: isLockedByOther ? T.textSubtle : T.accent,
              cursor: isLockedByOther ? "not-allowed" : "pointer",
            }}
          >
            ▶ Run autopilot
          </button>
        )}
      </div>

      <div
        style={{
          fontSize: 14,
          color: T.text,
          marginBottom: 22,
          lineHeight: 1.45,
          // Long parity-issue titles like [Shadow-Validation-Diff][integ] …
          // need to wrap cleanly without leaking past the detail pane.
          wordBreak: "break-word",
          overflowWrap: "anywhere",
        }}
      >
        {leaf.title}
      </div>

      <SectionTitle>Linked PRs</SectionTitle>
      <div style={{ marginBottom: 26 }}>
        {leaf.linkedPRs.length === 0 ? (
          <div style={{ fontSize: 12, color: T.textSubtle, fontStyle: "italic" }}>
            No linked PRs yet.
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {leaf.linkedPRs.map((pr) => (
              <a
                key={`${pr.repo}#${pr.number}`}
                href={pr.url}
                target="_blank"
                rel="noreferrer"
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 12,
                  padding: "10px 14px",
                  background: T.bgElev,
                  border: `1px solid ${T.border}`,
                  borderRadius: 8,
                  color: T.text,
                  textDecoration: "none",
                  fontSize: 13,
                }}
              >
                <span style={{ fontWeight: 700, color: T.accent }}>
                  {pr.repo}#{pr.number}
                </span>
                <span
                  style={{
                    fontSize: 11,
                    fontWeight: 600,
                    color: T.textMuted,
                    textTransform: "uppercase",
                    letterSpacing: 0.5,
                  }}
                >
                  {pr.state}
                </span>
                {pr.author && (
                  <span style={{ fontSize: 11, color: T.textSubtle }}>by @{pr.author}</span>
                )}
                {pr.mergedAt && (
                  <span style={{ fontSize: 11, color: T.textSubtle }}>
                    merged {pr.mergedAt.slice(0, 10)}
                  </span>
                )}
              </a>
            ))}
          </div>
        )}
      </div>

      <SectionTitle>Labels</SectionTitle>
      <div style={{ marginBottom: 26, display: "flex", flexWrap: "wrap", gap: 6 }}>
        {leaf.labels.length === 0 ? (
          <span style={{ fontSize: 12, color: T.textSubtle, fontStyle: "italic" }}>
            No labels.
          </span>
        ) : (
          leaf.labels.map((name) => (
            <span
              key={name}
              style={{
                fontSize: 11,
                padding: "3px 9px",
                background: T.codeBg,
                color: T.textMuted,
                border: `1px solid ${T.border}`,
                borderRadius: 999,
              }}
            >
              {name}
            </span>
          ))
        )}
      </div>

      <SectionTitle>Body</SectionTitle>
      <pre
        style={{
          margin: 0,
          padding: "16px 18px",
          background: T.codeBg,
          border: `1px solid ${T.border}`,
          borderRadius: 8,
          fontSize: 12,
          lineHeight: 1.5,
          maxHeight: 520,
          overflow: "auto",
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
          color: T.text,
          fontFamily:
            "ui-monospace, SFMono-Regular, Menlo, Monaco, 'Cascadia Code', monospace",
        }}
      >
        {leaf.body || "(empty body)"}
      </pre>

      <div
        style={{
          marginTop: 26,
          fontSize: 11,
          color: T.textSubtle,
          display: "flex",
          gap: 14,
          flexWrap: "wrap",
        }}
      >
        <span>created {leaf.createdAt.slice(0, 10)}</span>
        <span>·</span>
        <span>parent #{leaf.parentTracking}</span>
        <span>·</span>
        <Link
          to={`/parity/${leaf.connector}`}
          style={{ color: T.accent, textDecoration: "none" }}
        >
          all {leaf.connector} leaves →
        </Link>
      </div>
    </div>
  );
}

function ParityEmptyState({ loading }: { loading: boolean }) {
  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        color: T.textMuted,
        fontSize: 13,
      }}
    >
      {loading ? "Loading tree…" : "Select a leaf from the left to view details."}
    </div>
  );
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <h2
      style={{
        fontSize: 11,
        fontWeight: 700,
        color: T.textMuted,
        textTransform: "uppercase",
        letterSpacing: 0.8,
        margin: "0 0 10px 0",
      }}
    >
      {children}
    </h2>
  );
}

// ─── Journey bar (bottom) ─────────────────────────────────────────────────

function ParityJourneyBar({ counts }: { counts: Record<string, number> }) {
  return (
    <div
      style={{
        borderTop: `1px solid ${T.border}`,
        background: T.bgElev,
        height: 38,
        display: "flex",
        alignItems: "center",
        gap: 16,
        padding: "0 22px",
        fontSize: 11,
        color: T.textMuted,
      }}
    >
      <span style={{ fontWeight: 700, color: T.textSubtle, letterSpacing: 0.4 }}>
        SUMMARY
      </span>
      <span>
        {counts.total} leaves · {counts["pr-merged"]} merged · {counts["pr-open"]} open ·{" "}
        {counts["no-pr"]} unstarted · {counts.blocked} blocked
      </span>
      <span style={{ flex: 1 }} />
      <span style={{ color: T.textSubtle }}>
        background refresh every 5 min (supervisor)
      </span>
    </div>
  );
}

// ─── Log panel (right) ────────────────────────────────────────────────────

function ParityLogPanel({
  lock,
  refreshedAt,
}: {
  lock: LockState;
  refreshedAt: string;
}) {
  return (
    <aside
      style={{
        width: 300,
        borderLeft: `1px solid ${T.border}`,
        background: T.bgRightHeader,
        display: "flex",
        flexDirection: "column",
      }}
    >
      <div
        style={{
          padding: "14px 18px",
          borderBottom: `1px solid ${T.border}`,
          display: "flex",
          alignItems: "center",
          gap: 8,
        }}
      >
        <div style={{ fontSize: 12, fontWeight: 700, color: T.text }}>Live logs</div>
        <span style={{ flex: 1 }} />
        <span style={{ fontSize: 10, color: T.textMuted }}>
          {lock.busy ? "active" : "idle"}
        </span>
      </div>
      <div
        style={{
          flex: 1,
          overflowY: "auto",
          padding: 18,
          fontSize: 12,
          color: T.textMuted,
          lineHeight: 1.5,
        }}
      >
        {lock.busy ? (
          <div>
            <div style={{ color: T.accent, fontWeight: 700, marginBottom: 6 }}>
              ▶ Heartbeat running
            </div>
            <div>leaf #{lock.leaf}</div>
            <div>{lock.dryRun ? "dry-run" : "live"}</div>
            {lock.startedAt && (
              <div style={{ marginTop: 8 }}>
                started {new Date(lock.startedAt).toLocaleTimeString()}
              </div>
            )}
            <div style={{ marginTop: 14, fontStyle: "italic", color: T.textSubtle }}>
              Output streams to the engine log file. Tail in another terminal:{" "}
              <code
                style={{
                  background: T.codeBg,
                  padding: "1px 6px",
                  borderRadius: 4,
                  fontSize: 11,
                }}
              >
                tail -f ~/.tenxgrace/parity-heartbeat.log
              </code>
            </div>
          </div>
        ) : (
          <div>
            <div style={{ marginBottom: 10 }}>
              No active heartbeat. Click <strong>▶ Run autopilot</strong> on a
              no-PR leaf to start a dry-run.
            </div>
            {refreshedAt && (
              <div style={{ fontSize: 11, color: T.textSubtle }}>
                Tree last refreshed at {refreshedAt.slice(11, 19)} UTC.
                <br />
                Supervisor refreshes every 5 min in the background.
              </div>
            )}
          </div>
        )}
      </div>
    </aside>
  );
}
