import { useEffect, useRef, useState } from "react";
import connectorsRaw from "../../data/connectors.json";
import type { Connector } from "../../types/connector";
import { T } from "../../theme";
import { DOC_TYPES, type DocType } from "./enums";
import { Field, Section, inputStyle } from "./shared";
import type { WizardAction, WizardState, DiscoveryApiResult } from "./types";
import {
  DiscoveryProgressPanel,
  type DiscoveryEvent,
} from "./DiscoveryProgressPanel";

const connectors: Connector[] = connectorsRaw as Connector[];

const STREAM_EVENT_CAP = 1000;
const STREAM_EVENT_TRIM = 800;
const ACTIVE_DISCOVERY_VERSION = 1;
/** Drop persisted in-flight discovery state older than this (12 hours).
 * Server keeps job state only 60s after done, so anything older is stale.
 * Generous ceiling lets the user resume even after a long break. */
const ACTIVE_DISCOVERY_MAX_AGE_MS = 12 * 60 * 60_000;

interface PersistedDiscovery {
  version: number;
  jobId: string;
  connectorName: string;
  startedAt: number;
  events: DiscoveryEvent[];
}

function activeDiscoveryKey(sessionId: string | undefined): string | null {
  if (!sessionId) return null;
  return `grace:active-discovery:${sessionId}`;
}

function loadActiveDiscovery(sessionId: string | undefined): PersistedDiscovery | null {
  const key = activeDiscoveryKey(sessionId);
  if (!key) return null;
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<PersistedDiscovery>;
    if (parsed.version !== ACTIVE_DISCOVERY_VERSION) return null;
    if (!parsed.jobId || !parsed.startedAt || !Array.isArray(parsed.events)) return null;
    if (Date.now() - parsed.startedAt > ACTIVE_DISCOVERY_MAX_AGE_MS) {
      localStorage.removeItem(key);
      return null;
    }
    return parsed as PersistedDiscovery;
  } catch {
    return null;
  }
}

function clearActiveDiscovery(sessionId: string | undefined) {
  const key = activeDiscoveryKey(sessionId);
  if (!key) return;
  try { localStorage.removeItem(key); } catch { /* ignore */ }
}

export function IntegrationStepBasics({
  state,
  dispatch,
  sessionId,
}: {
  state: WizardState;
  dispatch: (a: WizardAction) => void;
  sessionId?: string;
}) {
  const set = (patch: Partial<WizardState>) => dispatch({ type: "set", patch });
  const collision = connectors.some(
    (c) =>
      c.name.toLowerCase() === state.connectorName.trim().toLowerCase() &&
      state.connectorName.trim().length > 0,
  );

  const canDiscover =
    state.connectorName.trim().length > 0 && state.discoveryStatus !== "running";

  const [events, setEvents] = useState<DiscoveryEvent[]>([]);
  const [panelOpen, setPanelOpen] = useState(true);
  const esRef = useRef<EventSource | null>(null);
  const startedAtRef = useRef<number | null>(null);
  /** Tracks the jobId of the currently-streaming discovery so persistence
   * + handler closures can reference it without an extra useState round-trip. */
  const activeJobIdRef = useRef<string | null>(null);
  /** True while `runDiscovery`'s POST is still in flight. When the SSE `done`
   * event fires we defer to the POST resolution to dispatch the terminal
   * state (it has the actual DiscoveryResult). When this is false and a
   * `done` arrives, we know the POST was abandoned (user navigated away
   * mid-run) and the result is unrecoverable — dispatch a terminal error
   * so the UI stops saying "Discovering…" forever. */
  const pendingPostRef = useRef(false);
  /** Highest ts seen so far. Seeded from the LS-restored backlog so the
   * server's SSE replay (which has the same ts'd events) is skipped instead
   * of being appended a second time. */
  const lastEventTsRef = useRef(0);

  const isDuplicate = (ts: number | undefined): boolean => {
    if (typeof ts !== "number") return false;
    if (ts <= lastEventTsRef.current) return true;
    lastEventTsRef.current = ts;
    return false;
  };

  /**
   * Open an EventSource for the given jobId and wire its line/progress/done
   * listeners. Pulled out so both a fresh `runDiscovery()` and a remount
   * `resumeDiscovery()` use the same listener setup.
   */
  const openStream = (jobId: string) => {
    esRef.current?.close();
    const es = new EventSource(`/api/discover-connector/stream/${jobId}`);
    esRef.current = es;
    activeJobIdRef.current = jobId;
    es.addEventListener("line", (e) => {
      try {
        const { line, ts } = JSON.parse((e as MessageEvent).data) as {
          line: string;
          ts?: number;
        };
        if (isDuplicate(ts)) return;
        appendEvent({ kind: "line", text: line, ts });
      } catch {
        /* ignore malformed event */
      }
    });
    es.addEventListener("progress", (e) => {
      try {
        const { message, ts } = JSON.parse((e as MessageEvent).data) as {
          message: string;
          ts?: number;
        };
        if (isDuplicate(ts)) return;
        appendEvent({ kind: "progress", text: message, ts });
      } catch {
        /* ignore */
      }
    });
    es.addEventListener("done", async (e) => {
      es.close();
      if (esRef.current === es) esRef.current = null;
      clearActiveDiscovery(sessionId);
      activeJobIdRef.current = null;
      if (pendingPostRef.current) {
        // The original POST is still in flight — it owns the terminal
        // dispatch (it has the full DiscoveryResult). Don't preempt it.
        return;
      }
      // Resumed-after-navigate case: original POST was abandoned. Try to
      // fetch the cached result from the server (we cache it for 60s after
      // discoveryFinish). If the result is recoverable, dispatch it like a
      // normal completion. Only fall back to discoveryError if we genuinely
      // can't recover.
      try {
        const r = await fetch(`/api/discover-connector/result/${jobId}`);
        if (r.ok) {
          const body = (await r.json()) as {
            ok?: boolean;
            result?: DiscoveryApiResult;
            error?: string;
            cancelled?: boolean;
          };
          if (body.cancelled) {
            dispatch({ type: "discoveryCancel" });
            return;
          }
          if (body.ok === false) {
            dispatch({
              type: "discoveryError",
              error: body.error ?? "Discovery failed while you were away.",
            });
            return;
          }
          if (body.ok && body.result) {
            dispatch({ type: "discoveryResult", result: body.result });
            dispatch({ type: "next" });
            return;
          }
        }
      } catch {
        /* fall through to error */
      }
      // Fall back to parsing the done payload directly.
      let payload: { ok?: boolean; error?: string; cancelled?: boolean } = {};
      try { payload = JSON.parse((e as MessageEvent).data); } catch { /* defaults */ }
      if (payload.cancelled) {
        dispatch({ type: "discoveryCancel" });
      } else if (payload.ok === false) {
        dispatch({
          type: "discoveryError",
          error: payload.error ?? "Discovery failed while you were away.",
        });
      } else {
        dispatch({
          type: "discoveryError",
          error:
            "Discovery completed but the result was not cached. Click Re-run discovery to fetch the populated fields.",
        });
      }
    });
    return es;
  };

  // Hydrate from localStorage on mount. If a discovery for this session was
  // in flight when we left, restore its events log + reconnect to the SSE
  // channel so the user picks up where they left off. The server replays
  // its logTail on subscribe, so even if our local events array is empty
  // we still see ~500 lines of context.
  useEffect(() => {
    const persisted = loadActiveDiscovery(sessionId);
    if (!persisted) return;
    setEvents(persisted.events);
    // Seed dedupe cursor so the server's SSE replay (same ts values) is
    // skipped instead of duplicating the restored backlog.
    lastEventTsRef.current = persisted.events.reduce(
      (max, e) => (typeof e.ts === "number" && e.ts > max ? e.ts : max),
      0,
    );
    setPanelOpen(true);
    startedAtRef.current = persisted.startedAt;
    dispatch({ type: "discoveryStart", jobId: persisted.jobId });
    if (persisted.connectorName && !state.connectorName) {
      set({ connectorName: persisted.connectorName });
    }
    openStream(persisted.jobId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    return () => {
      esRef.current?.close();
      esRef.current = null;
    };
  }, []);

  /**
   * Last-wall-clock time we flushed to localStorage. We rate-limit writes to
   * at most one per 500 ms (cheap, but no need to churn on every line) AND
   * always write the final batch via the status-change effect below.
   */
  const lastPersistRef = useRef(0);

  const persistEvents = (nextEvents: DiscoveryEvent[]) => {
    const key = activeDiscoveryKey(sessionId);
    if (!key) return;
    const jobId = activeJobIdRef.current;
    if (!jobId) return;
    const now = Date.now();
    if (now - lastPersistRef.current < 500) return;
    lastPersistRef.current = now;
    const writeWith = (eventsToWrite: DiscoveryEvent[]) => {
      const payload: PersistedDiscovery = {
        version: ACTIVE_DISCOVERY_VERSION,
        jobId,
        connectorName: state.connectorName.trim(),
        startedAt: startedAtRef.current ?? Date.now(),
        events: eventsToWrite,
      };
      localStorage.setItem(key, JSON.stringify(payload));
    };
    try {
      writeWith(nextEvents);
    } catch {
      // Quota exceeded — drop oldest half and retry once.
      try {
        writeWith(nextEvents.slice(-Math.floor(nextEvents.length / 2)));
      } catch {
        /* give up silently */
      }
    }
  };

  const appendEvent = (evt: DiscoveryEvent) => {
    setEvents((prev) => {
      const next = prev.length >= STREAM_EVENT_CAP
        ? [...prev.slice(prev.length - STREAM_EVENT_TRIM + 1), evt]
        : [...prev, evt];
      // Persist the new array synchronously here — the previous effect-based
      // debounce was getting clobbered by back-to-back event arrivals and
      // never fired, so localStorage was always empty during fast streams.
      persistEvents(next);
      return next;
    });
  };

  // Note: we deliberately do NOT have a status-change effect that clears
  // localStorage when status leaves "running". That would fire on every
  // initial mount (status starts "idle") and wipe the persisted entry
  // BEFORE the hydrate effect can read it. Instead, terminal clearing
  // happens explicitly in the SSE done handler and in the runDiscovery
  // success/error/cancel paths.

  const runDiscovery = async () => {
    const name = state.connectorName.trim();
    if (!name) return;
    const jobId = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

    // Reset per-run state.
    clearActiveDiscovery(sessionId);
    setEvents([]);
    setPanelOpen(true);
    startedAtRef.current = Date.now();
    pendingPostRef.current = true;

    dispatch({ type: "discoveryStart", jobId });

    // Open the SSE channel BEFORE the POST so we don't miss early events.
    // The server lazy-creates the job state if the GET arrives first.
    const es = openStream(jobId);

    try {
      const resp = await fetch("/api/discover-connector", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ name, jobId }),
      });
      const body = await resp.json();
      if (!resp.ok) {
        if (body?.cancelled) {
          dispatch({ type: "discoveryCancel" });
        } else {
          dispatch({
            type: "discoveryError",
            error: body?.error ?? `Discovery failed (${resp.status})`,
          });
        }
        return;
      }
      const result = body.result as DiscoveryApiResult;
      dispatch({ type: "discoveryResult", result });
      // Advance to Step 2 so the user immediately sees the populated fields.
      dispatch({ type: "next" });
    } catch (err) {
      dispatch({
        type: "discoveryError",
        error: (err as Error).message ?? "Network error",
      });
    } finally {
      // POST has settled — clear the "expecting POST" guard so any
      // late-arriving SSE `done` for a resumed session is treated correctly.
      pendingPostRef.current = false;
      // Safety net: if the server died mid-stream and never sent `done`,
      // close the EventSource so the browser doesn't keep auto-reconnecting.
      setTimeout(() => {
        if (esRef.current === es) {
          es.close();
          esRef.current = null;
        }
      }, 5000);
    }
  };

  const cancelDiscovery = () => {
    if (!state.discoveryJobId) return;
    fetch(`/api/discover-connector/cancel/${state.discoveryJobId}`, {
      method: "POST",
    }).catch(() => {
      /* swallow — the POST handler will still dispatch the terminal state */
    });
  };

  return (
    <div>
      <Section
        title="Auto-Discover (Recommended)"
        description="Let the LLM search the web for this connector's API docs and fill the technical fields automatically. You'll review and correct everything in the next step. Falls back to manual entry if discovery fails."
      >
        <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
          <button
            type="button"
            onClick={runDiscovery}
            disabled={!canDiscover}
            style={{
              padding: "12px 22px",
              borderRadius: 8,
              border: "none",
              background:
                state.discoveryStatus === "running"
                  ? T.warn
                  : canDiscover
                  ? T.accent
                  : T.border,
              color: "#fff",
              fontWeight: 700,
              fontSize: 14,
              cursor: canDiscover ? "pointer" : "not-allowed",
              display: "inline-flex",
              alignItems: "center",
              gap: 10,
            }}
          >
            {state.discoveryStatus === "running" ? (
              <>
                <span style={{ animation: "spin 1s linear infinite" }}>⟳</span>
                Discovering…
              </>
            ) : state.discoveryStatus === "done" ? (
              <>✓ Re-run discovery</>
            ) : (
              <>🔎 Discover from web</>
            )}
          </button>
          <span style={{ fontSize: 12, color: T.textMuted }}>
            Enter the connector name above first. Discovery takes ~30-90 seconds.
          </span>
        </div>
        {(state.discoveryStatus === "running" || events.length > 0) && (
          <DiscoveryProgressPanel
            status={state.discoveryStatus}
            events={events}
            open={panelOpen}
            onToggle={() => setPanelOpen((v) => !v)}
            onCancel={cancelDiscovery}
            startedAt={startedAtRef.current}
          />
        )}
        {state.discoveryStatus === "done" && (
          <div
            style={{
              marginTop: 12,
              padding: "10px 14px",
              borderRadius: 6,
              background: T.successSoft,
              color: T.success,
              fontSize: 13,
              border: `1px solid ${T.success}`,
            }}
          >
            ✓ Discovery complete. {Object.keys(state.discoveryMeta).length} field(s) populated.
            {state.discoveryNotes && (
              <div style={{ marginTop: 6, color: T.textMuted, fontSize: 12 }}>
                Notes: {state.discoveryNotes}
              </div>
            )}
          </div>
        )}
        {state.discoveryStatus === "error" && state.discoveryError && (
          <div
            style={{
              marginTop: 12,
              padding: "10px 14px",
              borderRadius: 6,
              background: T.errorSoft,
              color: T.error,
              fontSize: 13,
              border: `1px solid ${T.error}`,
            }}
          >
            ✕ Discovery failed: {state.discoveryError}. You can fill the fields manually using "Next →".
          </div>
        )}
      </Section>
      <Section
        title="Connector Identity"
        description="Required. Connector name drives branch naming, Rust module names, and search queries downstream."
      >
        <Field
          label="Connector name"
          required
          hint="PascalCase recommended (e.g. Stripe, Adyen). Case-sensitive in grace techspec."
        >
          <input
            value={state.connectorName}
            onChange={(e) => set({ connectorName: e.target.value })}
            placeholder="e.g. Stripe"
            style={inputStyle}
          />
          {collision && (
            <div
              style={{
                marginTop: 6,
                padding: "6px 10px",
                borderRadius: 4,
                background: T.warnSoft,
                color: T.warn,
                fontSize: 12,
                border: `1px solid ${T.warn}`,
              }}
            >
              ⚠ A connector named “{state.connectorName.trim()}” already exists in
              connectors.json. You can proceed if this is intentional.
            </div>
          )}
        </Field>
        <Field label="Display name" hint="Optional. Used for UI labels.">
          <input
            value={state.displayName}
            onChange={(e) => set({ displayName: e.target.value })}
            placeholder="e.g. Stripe (US)"
            style={inputStyle}
          />
        </Field>
        <Field
          label="Description"
          required
          hint="What this connector is, who it serves, what's being integrated."
        >
          <textarea
            value={state.description}
            onChange={(e) => set({ description: e.target.value })}
            placeholder="Stripe is a payment processor serving merchants across..."
            rows={4}
            style={{ ...inputStyle, resize: "vertical", fontFamily: "inherit" }}
          />
        </Field>
      </Section>

      <Section
        title="Primary High-Quality Docs"
        description="If you have any of these, L2 Links Agent uses them directly and skips general web search."
      >
        <Field label="OpenAPI / Swagger URL" hint="Highest priority source.">
          <input
            value={state.openApiUrl}
            onChange={(e) => set({ openApiUrl: e.target.value })}
            placeholder="https://docs.example.com/openapi.yaml"
            style={inputStyle}
          />
        </Field>
        <Field label="Postman Collection URL">
          <input
            value={state.postmanCollectionUrl}
            onChange={(e) => set({ postmanCollectionUrl: e.target.value })}
            placeholder="https://www.postman.com/..."
            style={inputStyle}
          />
        </Field>
        <Field label="Integration Guide URL" hint="Tertiary source.">
          <input
            value={state.integrationGuideUrl}
            onChange={(e) => set({ integrationGuideUrl: e.target.value })}
            placeholder="https://docs.example.com/getting-started"
            style={inputStyle}
          />
        </Field>
      </Section>

      <Section
        title="Additional Documentation"
        description="Per-topic links (auth guide, webhook guide, error reference, etc.). At least one api_reference is required if OpenAPI/Postman aren't provided."
      >
        <DocsEditor state={state} dispatch={dispatch} />
      </Section>
    </div>
  );
}

function DocsEditor({
  state,
  dispatch,
}: {
  state: WizardState;
  dispatch: (a: WizardAction) => void;
}) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      {state.docs.map((doc, i) => (
        <div
          key={i}
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 2fr 1fr auto",
            gap: 6,
            alignItems: "center",
          }}
        >
          <input
            value={doc.title}
            onChange={(e) =>
              dispatch({ type: "updateDoc", index: i, patch: { title: e.target.value } })
            }
            placeholder="Title"
            style={inputStyle}
          />
          <input
            value={doc.url}
            onChange={(e) =>
              dispatch({ type: "updateDoc", index: i, patch: { url: e.target.value } })
            }
            placeholder="https://…"
            style={inputStyle}
          />
          <select
            value={doc.type}
            onChange={(e) =>
              dispatch({
                type: "updateDoc",
                index: i,
                patch: { type: e.target.value as DocType },
              })
            }
            style={inputStyle}
          >
            {DOC_TYPES.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
          <button
            type="button"
            onClick={() => dispatch({ type: "removeDoc", index: i })}
            style={{
              padding: "0 10px",
              border: `1px solid ${T.border}`,
              borderRadius: 6,
              background: T.bg,
              color: T.textMuted,
              cursor: "pointer",
              fontSize: 13,
              minHeight: 36,
            }}
          >
            ✕
          </button>
        </div>
      ))}
      <button
        type="button"
        onClick={() =>
          dispatch({
            type: "addDoc",
            doc: { title: "", url: "", type: "api_reference" },
          })
        }
        style={{
          padding: "8px 14px",
          borderRadius: 6,
          border: `1px dashed ${T.border}`,
          background: "transparent",
          color: T.textMuted,
          cursor: "pointer",
          fontWeight: 600,
          fontSize: 13,
          alignSelf: "flex-start",
        }}
      >
        + Add doc URL
      </button>
    </div>
  );
}
