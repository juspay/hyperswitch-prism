import { useEffect, useState } from "react";
import { T } from "../theme";
import { PIPELINE } from "../hooks/usePipeline";

interface CheckpointHistory {
  checkpointId: string;
  status: string;
  updatedAt: number;
}
interface RunSummary {
  runId: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  lastCheckpoint?: string;
  lastStatus?: string;
  checkpoints?: CheckpointHistory[];
}

export function RunsPicker({
  currentRunId,
  onRequestList,
  onResume,
  onNewRun,
  runs,
}: {
  currentRunId?: string;
  onRequestList: () => void;
  onResume: (runId: string, startFrom?: string) => void;
  onNewRun: () => void;
  runs: RunSummary[];
}) {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (open) onRequestList();
  }, [open]);

  return (
    <div style={{ position: "relative" }}>
      <button
        onClick={() => setOpen((o) => !o)}
        style={{
          padding: "7px 14px",
          borderRadius: 6,
          border: `1px solid ${T.borderStrong}`,
          background: T.bgElev,
          color: T.text,
          fontWeight: 600,
          fontSize: 12,
          cursor: "pointer",
        }}
      >
        Past runs ▾
      </button>
      {open && (
        <div
          style={{
            position: "absolute",
            top: "calc(100% + 6px)",
            right: 0,
            width: 460,
            maxHeight: 520,
            overflowY: "auto",
            background: T.bgElev,
            border: `1px solid ${T.border}`,
            borderRadius: 10,
            boxShadow: T.shadowLg,
            zIndex: 10,
          }}
        >
          <div
            style={{
              padding: "12px 16px",
              borderBottom: `1px solid ${T.border}`,
              fontSize: 11,
              fontWeight: 700,
              letterSpacing: 0.6,
              textTransform: "uppercase",
              color: T.textMuted,
              display: "flex",
              justifyContent: "space-between",
            }}
          >
            <span>Saved runs ({runs.length})</span>
            <button
              onClick={() => setOpen(false)}
              style={{
                background: "none",
                border: "none",
                color: T.textMuted,
                fontSize: 14,
                cursor: "pointer",
                padding: 0,
                lineHeight: 1,
              }}
            >
              ×
            </button>
          </div>
          <div
            style={{
              padding: "10px 16px",
              borderBottom: `1px solid ${T.border}`,
            }}
          >
            <button
              onClick={() => {
                onNewRun();
                setOpen(false);
              }}
              style={{
                width: "100%",
                padding: "8px 12px",
                borderRadius: 6,
                border: `1px solid ${T.accent}`,
                background: T.accent,
                color: "#fff",
                fontSize: 12,
                fontWeight: 700,
                cursor: "pointer",
              }}
              title="Abandon the current run state and start a fresh one. Engine restarts."
            >
              + New run
            </button>
          </div>
          {runs.length === 0 && (
            <div
              style={{
                padding: "24px 16px",
                color: T.textSubtle,
                fontSize: 13,
                textAlign: "center",
              }}
            >
              No saved runs yet.
            </div>
          )}
          {runs.map((r) => {
            const isCurrent = r.runId === currentRunId;
            const latestStatus: Record<string, string> = {};
            const latestUpdatedAt: Record<string, number> = {};
            for (const c of r.checkpoints ?? []) {
              if (
                latestUpdatedAt[c.checkpointId] === undefined ||
                c.updatedAt >= latestUpdatedAt[c.checkpointId]!
              ) {
                latestStatus[c.checkpointId] = c.status;
                latestUpdatedAt[c.checkpointId] = c.updatedAt;
              }
            }
            const passedIds = new Set(
              Object.entries(latestStatus)
                .filter(([, s]) => s === "passed")
                .map(([id]) => id)
            );
            const passedCount = passedIds.size;
            const statusColor = (s?: string) =>
              s === "passed"
                ? T.success
                : s === "running"
                  ? T.accent
                  : s === "failed"
                    ? T.error
                    : T.border;
            const lastPassedIdx = PIPELINE.map((p) => p.id).reduce(
              (acc, id, idx) => (passedIds.has(id) ? idx : acc),
              -1
            );
            const furthest =
              PIPELINE.find(
                (p) =>
                  latestStatus[p.id] === "running" ||
                  latestStatus[p.id] === "failed"
              ) ??
              (lastPassedIdx >= 0 && lastPassedIdx < PIPELINE.length - 1
                ? PIPELINE[lastPassedIdx + 1]
                : lastPassedIdx === PIPELINE.length - 1
                  ? PIPELINE[lastPassedIdx]
                  : undefined);
            const furthestStatus = furthest
              ? latestStatus[furthest.id] ?? "idle"
              : undefined;
            return (
              <div
                key={r.runId}
                onClick={() => {
                  if (!isCurrent) {
                    onResume(r.runId);
                    setOpen(false);
                  }
                }}
                style={{
                  padding: "12px 16px",
                  borderBottom: `1px solid ${T.border}`,
                  background: isCurrent ? T.accentSoft : "transparent",
                  cursor: isCurrent ? "default" : "pointer",
                }}
              >
                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "baseline",
                    gap: 10,
                  }}
                >
                  <div
                    style={{
                      fontSize: 13,
                      fontWeight: 600,
                      color: T.text,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                      flex: 1,
                      minWidth: 0,
                    }}
                    title={r.title}
                  >
                    {r.title || "(no title)"}
                  </div>
                  {isCurrent && (
                    <span
                      style={{
                        fontSize: 9,
                        fontWeight: 700,
                        color: T.accent,
                        background: T.bgElev,
                        padding: "2px 6px",
                        borderRadius: 999,
                        textTransform: "uppercase",
                        letterSpacing: 0.5,
                      }}
                    >
                      Current
                    </span>
                  )}
                </div>
                <div
                  style={{
                    fontSize: 10,
                    color: T.textSubtle,
                    fontFamily: "ui-monospace, monospace",
                    marginTop: 2,
                  }}
                >
                  {r.runId}
                </div>
                <div
                  style={{
                    fontSize: 11,
                    color: T.textMuted,
                    marginTop: 4,
                  }}
                >
                  {new Date(r.updatedAt).toLocaleString()} ·{" "}
                  <span style={{ color: T.success, fontWeight: 600 }}>
                    {passedCount}
                  </span>{" "}
                  / {PIPELINE.length} stages passed
                </div>
                <div
                  style={{
                    marginTop: 6,
                    display: "flex",
                    gap: 3,
                    alignItems: "center",
                  }}
                  title={PIPELINE.map(
                    (p) => `${p.id}: ${latestStatus[p.id] ?? "idle"}`
                  ).join("\n")}
                >
                  {PIPELINE.map((p) => (
                    <div
                      key={p.id}
                      style={{
                        flex: 1,
                        height: 4,
                        borderRadius: 2,
                        background: statusColor(latestStatus[p.id]),
                      }}
                    />
                  ))}
                </div>
                {furthest && (
                  <div
                    style={{
                      fontSize: 10,
                      color: T.textSubtle,
                      marginTop: 4,
                    }}
                  >
                    {furthestStatus === "running"
                      ? "▶ at"
                      : furthestStatus === "failed"
                        ? "✕ failed at"
                        : passedCount === PIPELINE.length
                          ? "✓ finished"
                          : "· up to"}{" "}
                    <span style={{ color: T.textMuted, fontWeight: 600 }}>
                      {furthest.name}
                    </span>
                  </div>
                )}
                <div
                  style={{
                    marginTop: 10,
                    display: "flex",
                    gap: 6,
                    flexWrap: "wrap",
                    alignItems: "center",
                  }}
                >
                  <button
                    onClick={(ev) => {
                      ev.stopPropagation();
                      onResume(r.runId);
                      setOpen(false);
                    }}
                    style={{
                      fontSize: 11,
                      fontWeight: 600,
                      padding: "5px 10px",
                      borderRadius: 6,
                      border: `1px solid ${T.accent}`,
                      background: T.accent,
                      color: "#fff",
                      cursor: "pointer",
                    }}
                    title="Auto-resume at the first non-passed stage"
                  >
                    ↻ Resume
                  </button>
                  <span
                    style={{
                      fontSize: 10,
                      color: T.textSubtle,
                      marginLeft: 4,
                    }}
                  >
                    or restart from:
                  </span>
                  {PIPELINE.map((p) => {
                    const st = latestStatus[p.id] ?? "idle";
                    const isPassed = st === "passed";
                    const isRunning = st === "running";
                    const isFailed = st === "failed";
                    const bg = isPassed
                      ? T.successSoft
                      : isRunning
                        ? T.accentSoft
                        : isFailed
                          ? T.errorSoft
                          : T.bg;
                    const border = isPassed
                      ? T.success
                      : isRunning
                        ? T.accent
                        : isFailed
                          ? T.error
                          : T.border;
                    const color = isPassed
                      ? T.success
                      : isRunning
                        ? T.accent
                        : isFailed
                          ? T.error
                          : T.textMuted;
                    return (
                      <button
                        key={p.id}
                        onClick={(ev) => {
                          ev.stopPropagation();
                          onResume(r.runId, p.id);
                          setOpen(false);
                        }}
                        style={{
                          fontSize: 10,
                          padding: "3px 7px",
                          borderRadius: 4,
                          border: `1px solid ${border}`,
                          background: bg,
                          color,
                          cursor: "pointer",
                          fontWeight: isRunning || isFailed ? 600 : 500,
                        }}
                        title={`Restart this run at "${p.name}" (current status: ${st})`}
                      >
                        {p.id}
                      </button>
                    );
                  })}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
