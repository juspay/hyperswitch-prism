import connectorsRaw from "../../data/connectors.json";
import type { Connector } from "../../types/connector";
import { T } from "../../theme";
import { DOC_TYPES, type DocType } from "./enums";
import { Field, Section, inputStyle } from "./shared";
import type { WizardAction, WizardState, DiscoveryApiResult } from "./types";

const connectors: Connector[] = connectorsRaw as Connector[];

export function IntegrationStepBasics({
  state,
  dispatch,
}: {
  state: WizardState;
  dispatch: (a: WizardAction) => void;
}) {
  const set = (patch: Partial<WizardState>) => dispatch({ type: "set", patch });
  const collision = connectors.some(
    (c) =>
      c.name.toLowerCase() === state.connectorName.trim().toLowerCase() &&
      state.connectorName.trim().length > 0,
  );

  const canDiscover =
    state.connectorName.trim().length > 0 && state.discoveryStatus !== "running";

  const runDiscovery = async () => {
    const name = state.connectorName.trim();
    if (!name) return;
    const jobId = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    dispatch({ type: "discoveryStart", jobId });
    try {
      const resp = await fetch("/api/discover-connector", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ name }),
      });
      const body = await resp.json();
      if (!resp.ok) {
        dispatch({
          type: "discoveryError",
          error: body?.error ?? `Discovery failed (${resp.status})`,
        });
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
    }
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
