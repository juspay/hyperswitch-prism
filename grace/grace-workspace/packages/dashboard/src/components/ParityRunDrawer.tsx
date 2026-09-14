import { useEffect, useMemo, useRef, useState } from "react";
import { T } from "../theme";
import type { ParityLeaf } from "../types/parity";

type PhaseId =
  | "discover" | "refresh" | "sweep" | "decide"
  | "claim" | "understand" | "plan" | "execute" | "verify" | "handoff";

type PhaseStatus = "pending" | "running" | "ok" | "skipped" | "fail" | "raced";

interface PhaseState {
  status: PhaseStatus;
  detail?: string;
  markdown?: string;
  branch?: string;
  target?: string;
  locus?: string;
}

const PHASE_ORDER: PhaseId[] = [
  "discover", "refresh", "decide",
  "claim", "understand", "plan", "execute", "verify", "handoff",
];

const PHASE_LABELS: Record<PhaseId, string> = {
  discover: "Discover tree",
  refresh: "Refresh dashboard",
  sweep: "Sweep open PRs",
  decide: "Decide leaf",
  claim: "Claim issue",
  understand: "Understand",
  plan: "Plan",
  execute: "Execute",
  verify: "gRPC verify",
  handoff: "Handoff (PR)",
};

interface ProgressEvent {
  phase: string;
  status?: string;
  outcome?: string;
  detail?: string;
  picked?: number;
  reason?: string;
  confidence?: string;
  locus?: string;
  markdown?: string;
  target?: string;
  branch?: string;
  tail?: string;
  prUrl?: string;
  step?: string;
  blocker?: string;
}

type DrawerStage = "idle" | "streaming" | "preview-ready" | "live-streaming" | "live-done" | "failed" | "cancelled";

type RunnerChoice = "claude-code" | "opencode";

export function ParityRunDrawer({ leaf, onClose, onLockChange }: { leaf: ParityLeaf; onClose: () => void; onLockChange?: () => void }) {
  const [stage, setStage] = useState<DrawerStage>("idle");
  const [phases, setPhases] = useState<Record<PhaseId, PhaseState>>(() => initPhases());
  const [tailLog, setTailLog] = useState<string[]>([]);
  const [outcome, setOutcome] = useState<string | null>(null);
  const [outcomeDetail, setOutcomeDetail] = useState<string | null>(null);
  const [activeDryRun, setActiveDryRun] = useState<boolean | null>(null);
  const [runner, setRunner] = useState<RunnerChoice>("claude-code");
  const esRef = useRef<EventSource | null>(null);

  function closeStream() {
    if (esRef.current) {
      try { esRef.current.close(); } catch { /* ignore */ }
      esRef.current = null;
    }
  }

  useEffect(() => () => closeStream(), []);

  async function startRun(dryRun: boolean) {
    closeStream();
    setPhases(initPhases());
    setTailLog([]);
    setOutcome(null);
    setOutcomeDetail(null);
    setActiveDryRun(dryRun);
    setStage(dryRun ? "streaming" : "live-streaming");

    let kicked = false;
    try {
      const res = await fetch(`/api/parity/run/${leaf.number}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ dryRun, runner }),
      });
      if (res.status === 409) {
        const data = await res.json();
        setOutcome("locked");
        setOutcomeDetail(`Another heartbeat is running (leaf #${data.current?.leaf}, dryRun=${data.current?.dryRun})`);
        setStage("failed");
        return;
      }
      if (!res.ok) {
        setOutcome("error");
        setOutcomeDetail(`POST /api/parity/run/${leaf.number} → HTTP ${res.status}`);
        setStage("failed");
        return;
      }
      kicked = true;
    } catch (e) {
      setOutcome("error");
      setOutcomeDetail(String(e));
      setStage("failed");
      return;
    }
    if (!kicked) return;
    if (onLockChange) onLockChange();

    const es = new EventSource(`/api/parity/stream/${leaf.number}`);
    esRef.current = es;

    es.addEventListener("hello", () => { /* connected */ });
    es.addEventListener("progress", (ev) => {
      try {
        const data: ProgressEvent = JSON.parse((ev as MessageEvent).data);
        applyProgress(data);
      } catch { /* ignore */ }
    });
    es.addEventListener("log", (ev) => {
      try {
        const line = JSON.parse((ev as MessageEvent).data) as string;
        setTailLog((prev) => {
          const next = [...prev, line];
          return next.length > 50 ? next.slice(-50) : next;
        });
      } catch { /* ignore */ }
    });
    es.addEventListener("done", (ev) => {
      try {
        const data = JSON.parse((ev as MessageEvent).data) as { code: number | null; signal?: string | null };
        finalize(data.code, data.signal ?? null, dryRun);
      } catch {
        finalize(null, null, dryRun);
      }
    });
    es.onerror = () => {
      // EventSource will auto-reconnect; ignore unless we're already done
    };
  }

  function applyProgress(ev: ProgressEvent) {
    setPhases((prev) => {
      const next = { ...prev };
      const id = ev.phase as PhaseId;
      if (!(id in PHASE_LABELS)) return prev;
      const cur = next[id] ?? { status: "pending" };
      let status: PhaseStatus = cur.status;
      if (ev.status === "start") status = "running";
      else if (ev.status === "ok") status = "ok";
      else if (ev.status === "fail") status = "fail";
      else if (ev.status === "skipped-dry-run") status = "skipped";
      else if (ev.status === "raced") status = "raced";
      next[id] = {
        ...cur,
        status,
        detail: ev.detail ?? ev.reason ?? cur.detail,
        markdown: ev.markdown ?? cur.markdown,
        branch: ev.branch ?? cur.branch,
        target: ev.target ?? cur.target,
        locus: ev.locus ?? cur.locus,
      };
      return next;
    });
    if (ev.phase === "done") {
      setOutcome(ev.outcome ?? null);
      setOutcomeDetail(ev.detail ?? null);
    }
  }

  function finalize(code: number | null, signal: string | null, dryRun: boolean) {
    closeStream();
    if (onLockChange) onLockChange();
    if (signal === "SIGTERM") {
      setStage("cancelled");
      return;
    }
    if (code !== 0) {
      setStage("failed");
      return;
    }
    if (dryRun) {
      setStage("preview-ready");
    } else {
      setStage("live-done");
    }
  }

  async function cancel() {
    await fetch("/api/parity/cancel", { method: "POST" });
  }

  const verifyOk = phases.verify?.status === "ok";
  const canPromoteLive = stage === "preview-ready" && verifyOk && (outcome === "pr-opened" || outcome === null);

  return (
    <div style={overlay} onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <aside style={drawer}>
        <header style={headerStyle}>
          <div>
            <div style={{ fontSize: 11, color: T.textMuted, fontWeight: 600, letterSpacing: 0.5, textTransform: "uppercase" }}>
              Run Autopilot · leaf #{leaf.number}
            </div>
            <div style={{ fontSize: 15, fontWeight: 700, color: T.text, marginTop: 4 }}>
              {leaf.connector} / {leaf.flow}
            </div>
            <div style={{ fontSize: 12, color: T.textMuted, marginTop: 2 }}>{leaf.title}</div>
          </div>
          <button onClick={onClose} style={iconBtn} aria-label="close">✕</button>
        </header>

        {stage === "idle" && (
          <section style={section}>
            <p style={{ fontSize: 13, color: T.text, margin: "0 0 12px" }}>
              <strong>Dry-run</strong> executes UNDERSTAND, PLAN, EXECUTE (cargo build/clippy/nextest), and gRPC VERIFY locally.
              It does <strong>not</strong> claim the issue, post comments, push commits, or open a PR.
              After preview, you can promote to a live run.
            </p>

            <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 12, flexWrap: "wrap" }}>
              <span style={{ fontSize: 12, color: T.textMuted, fontWeight: 600 }}>Runner:</span>
              <div style={{ display: "flex", gap: 4 }}>
                {(["claude-code", "opencode"] as const).map((r) => (
                  <button
                    key={r}
                    onClick={() => setRunner(r)}
                    aria-pressed={runner === r}
                    style={{
                      padding: "4px 10px",
                      fontSize: 11,
                      fontWeight: 600,
                      background: runner === r ? T.accent : T.bgElev,
                      color: runner === r ? "#fff" : T.text,
                      border: `1px solid ${T.border}`,
                      borderRadius: 4,
                      cursor: "pointer",
                    }}
                  >
                    {r}
                  </button>
                ))}
              </div>
              {runner === "opencode" && (
                <span style={{ fontSize: 10, color: T.warn }}>
                  requires <code style={{ background: T.codeBg, padding: "0 4px", borderRadius: 3 }}>opencode serve</code> on :4096; no cross-heartbeat resume
                </span>
              )}
            </div>

            <div style={{ display: "flex", gap: 8 }}>
              <button onClick={() => startRun(true)} style={primaryBtn}>▶ Dry-run</button>
              <button onClick={onClose} style={secondaryBtn}>Cancel</button>
            </div>
            <p style={{ fontSize: 11, color: T.textSubtle, marginTop: 16 }}>
              Heartbeat runs the LLM agent (token cost) + cargo (multi-minute) + boots grpc-server with creds.
              Only one runs at a time per dev server.
            </p>
          </section>
        )}

        {stage !== "idle" && (
          <>
            <section style={section}>
              <h3 style={h3}>Phase progress {activeDryRun ? "· dry-run" : "· LIVE"}</h3>
              <ol style={phaseList}>
                {PHASE_ORDER.map((id) => {
                  const ph = phases[id];
                  return (
                    <li key={id} style={phaseRow}>
                      <span style={{ fontSize: 14, width: 18 }}>{glyph(ph.status)}</span>
                      <span style={{ fontSize: 13, color: T.text, fontWeight: ph.status === "running" ? 600 : 500, flex: 1 }}>
                        {PHASE_LABELS[id]}
                      </span>
                      <span style={{ fontSize: 11, color: statusFg(ph.status), fontWeight: 600 }}>{statusLabel(ph.status)}</span>
                    </li>
                  );
                })}
              </ol>
            </section>

            {phases.understand?.locus === "hs-bridge" && (
              <section
                style={{
                  ...section,
                  background: T.warnSoft,
                  borderTop: `2px solid ${T.warn}`,
                  borderBottom: `2px solid ${T.warn}`,
                }}
              >
                <div style={{ fontSize: 12, color: T.text, lineHeight: 1.6 }}>
                  <strong>⚠ This fix targets the UCS bridge in <code style={inlineCodeStrong}>juspay/hyperswitch</code>, not prism.</strong>
                  <br />
                  Branch will be cut from <code style={inlineCodeStrong}>origin/main</code> of the configured bridge clone.
                  <br />
                  If you click <em>Run live →</em>, a draft PR will be opened against{" "}
                  <strong>juspay/hyperswitch</strong> (not <code style={inlineCodeStrong}>juspay/hyperswitch-prism</code>).
                </div>
              </section>
            )}

            {phases.understand?.markdown && (
              <section style={section}>
                <h3 style={h3}>Understanding Summary</h3>
                <Code value={phases.understand.markdown} />
              </section>
            )}

            {phases.plan?.markdown && (
              <section style={section}>
                <h3 style={h3}>Implementation Plan</h3>
                <Code value={phases.plan.markdown} />
              </section>
            )}

            {phases.execute && (phases.execute.branch || phases.execute.target) && (
              <section style={section}>
                <h3 style={h3}>Execute</h3>
                <div style={{ fontSize: 12, color: T.textMuted }}>
                  target: <code style={inlineCode}>{phases.execute.target}</code>{" "}
                  branch: <code style={inlineCode}>{phases.execute.branch}</code>
                </div>
              </section>
            )}

            {phases.verify?.markdown && (
              <section style={section}>
                <h3 style={h3}>gRPC Verification</h3>
                <Code value={phases.verify.markdown} />
              </section>
            )}

            <section style={section}>
              <h3 style={h3}>Tail log</h3>
              <pre style={tailStyle}>
                {tailLog.length === 0 ? "(no output yet)" : tailLog.join("\n")}
              </pre>
            </section>

            <footer style={footerStyle}>
              {(stage === "streaming" || stage === "live-streaming") && (
                <button onClick={cancel} style={dangerBtn}>Cancel run (SIGTERM)</button>
              )}
              {canPromoteLive && (
                <button onClick={() => startRun(false)} style={primaryBtn}>
                  Run live → claim + commit + push + open PR
                </button>
              )}
              {stage === "preview-ready" && !canPromoteLive && (
                <span style={{ fontSize: 12, color: T.error }}>
                  Verify did not pass — promoting to live is disabled.
                </span>
              )}
              {stage === "live-done" && (
                <span style={{ fontSize: 13, color: T.success, fontWeight: 600 }}>
                  ✓ Live run complete. {outcomeDetail ? <a href={outcomeDetail} target="_blank" rel="noreferrer" style={{ color: T.accent }}>{outcomeDetail}</a> : null}
                </span>
              )}
              {stage === "failed" && (
                <span style={{ fontSize: 13, color: T.error }}>✗ {outcome ?? "failed"}{outcomeDetail ? ` — ${outcomeDetail}` : ""}</span>
              )}
              {stage === "cancelled" && (
                <span style={{ fontSize: 13, color: T.warn }}>Cancelled by operator</span>
              )}
              <button onClick={onClose} style={secondaryBtn}>Close drawer</button>
            </footer>
          </>
        )}
      </aside>
    </div>
  );
}

function initPhases(): Record<PhaseId, PhaseState> {
  const out: Partial<Record<PhaseId, PhaseState>> = {};
  for (const id of PHASE_ORDER) out[id] = { status: "pending" };
  return out as Record<PhaseId, PhaseState>;
}

function glyph(s: PhaseStatus): string {
  switch (s) {
    case "ok": return "●";
    case "running": return "◐";
    case "fail": return "✗";
    case "raced": return "⚠";
    case "skipped": return "○";
    case "pending":
    default: return "○";
  }
}

function statusLabel(s: PhaseStatus): string {
  if (s === "ok") return "OK";
  if (s === "running") return "RUNNING";
  if (s === "fail") return "FAIL";
  if (s === "raced") return "RACED";
  if (s === "skipped") return "SKIPPED";
  return "—";
}

function statusFg(s: PhaseStatus): string {
  if (s === "ok") return T.success;
  if (s === "fail") return T.error;
  if (s === "raced") return T.warn;
  if (s === "running") return T.accent;
  return T.textSubtle;
}

function Code({ value }: { value: string }) {
  return (
    <pre style={codeBlock}>{value}</pre>
  );
}

// ----- styles --------------------------------------------------------------

const overlay: React.CSSProperties = {
  position: "fixed",
  top: 0, left: 0, right: 0, bottom: 0,
  background: "rgba(0, 0, 0, 0.32)",
  zIndex: 100,
  display: "flex",
  justifyContent: "flex-end",
};
const drawer: React.CSSProperties = {
  width: "min(640px, 90vw)",
  height: "100vh",
  background: T.bg,
  borderLeft: `1px solid ${T.border}`,
  overflowY: "auto",
  boxShadow: "-6px 0 24px rgba(74, 53, 36, 0.18)",
};
const headerStyle: React.CSSProperties = {
  position: "sticky",
  top: 0,
  zIndex: 1,
  background: T.bgElev,
  borderBottom: `1px solid ${T.border}`,
  padding: "16px 20px",
  display: "flex",
  justifyContent: "space-between",
  gap: 12,
};
const iconBtn: React.CSSProperties = {
  background: "transparent",
  border: "none",
  fontSize: 18,
  color: T.textMuted,
  cursor: "pointer",
  padding: 4,
};
const section: React.CSSProperties = {
  padding: "16px 20px",
  borderBottom: `1px solid ${T.border}`,
};
const h3: React.CSSProperties = {
  margin: "0 0 10px",
  fontSize: 11,
  fontWeight: 700,
  color: T.textMuted,
  textTransform: "uppercase",
  letterSpacing: 0.4,
};
const phaseList: React.CSSProperties = { listStyle: "none", margin: 0, padding: 0 };
const phaseRow: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  padding: "6px 0",
};
const inlineCode: React.CSSProperties = {
  background: T.codeBg,
  padding: "1px 6px",
  borderRadius: 4,
  fontSize: 11,
};
const inlineCodeStrong: React.CSSProperties = {
  background: T.codeBg,
  padding: "1px 6px",
  borderRadius: 4,
  fontSize: 11,
  fontWeight: 700,
};
const codeBlock: React.CSSProperties = {
  background: T.codeBg,
  border: `1px solid ${T.border}`,
  borderRadius: 6,
  padding: 12,
  fontSize: 11.5,
  lineHeight: 1.5,
  maxHeight: 320,
  overflow: "auto",
  margin: 0,
  whiteSpace: "pre-wrap",
  fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
  color: T.text,
};
const tailStyle: React.CSSProperties = {
  ...codeBlock,
  maxHeight: 180,
  background: "#2d2418",
  color: "#f5ebd2",
  border: "none",
};
const footerStyle: React.CSSProperties = {
  position: "sticky",
  bottom: 0,
  background: T.bgElev,
  borderTop: `1px solid ${T.border}`,
  padding: "12px 20px",
  display: "flex",
  flexWrap: "wrap",
  gap: 10,
  alignItems: "center",
};
const primaryBtn: React.CSSProperties = {
  background: T.accent,
  color: "#fff",
  border: "none",
  borderRadius: 6,
  padding: "8px 14px",
  fontSize: 13,
  fontWeight: 600,
  cursor: "pointer",
};
const secondaryBtn: React.CSSProperties = {
  background: T.bgElev,
  color: T.text,
  border: `1px solid ${T.border}`,
  borderRadius: 6,
  padding: "8px 14px",
  fontSize: 13,
  fontWeight: 500,
  cursor: "pointer",
};
const dangerBtn: React.CSSProperties = {
  background: T.errorSoft,
  color: T.error,
  border: `1px solid ${T.error}`,
  borderRadius: 6,
  padding: "8px 14px",
  fontSize: 13,
  fontWeight: 600,
  cursor: "pointer",
};
