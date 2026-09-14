import { useState } from "react";
import { T } from "../../theme";
import type { WizardAction, WizardState } from "./types";

export function IntegrationStepReview({
  state,
  dispatch,
}: {
  state: WizardState;
  dispatch: (a: WizardAction) => void;
}) {
  const pms = Object.entries(state.selectedPMs).flatMap(([c, ms]) =>
    ms.map((m) => `${c} / ${m}`),
  );
  const [specExpanded, setSpecExpanded] = useState(false);

  const specReady = state.specMarkdown !== null;
  const specGenerating = state.discoveryStatus === "running";

  return (
    <div>
      {/* Tech spec status banner */}
      {(specReady || specGenerating) && (
        <div
          style={{
            background: specReady ? T.successSoft : T.warnSoft,
            color: specReady ? T.success : T.warn,
            border: `1px solid ${specReady ? T.success : T.warn}`,
            borderRadius: 6,
            padding: "10px 14px",
            fontSize: 12,
            marginBottom: 16,
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: 12,
          }}
        >
          <div>
            {specReady ? (
              <>
                <strong>✓ Tech spec ready</strong> ·{" "}
                {state.specMarkdown!.length.toLocaleString()} chars · will be written to
                <code style={{ marginLeft: 4 }}>
                  grace/rulesbook/codegen/references/specs/{state.connectorName}.md
                </code>{" "}
                so L2 skips its LLM phases.
              </>
            ) : (
              <>
                <strong>⟳ Generating tech spec…</strong> Launch is disabled until ready.
              </>
            )}
          </div>
          {specReady && (
            <button
              type="button"
              onClick={() => setSpecExpanded((v) => !v)}
              style={{
                padding: "4px 10px",
                border: `1px solid ${T.success}`,
                borderRadius: 4,
                background: "transparent",
                color: T.success,
                fontWeight: 600,
                fontSize: 11,
                cursor: "pointer",
              }}
            >
              {specExpanded ? "Hide spec" : "Show spec"}
            </button>
          )}
        </div>
      )}
      {specReady && specExpanded && (
        <pre
          style={{
            background: T.codeBg,
            border: `1px solid ${T.border}`,
            borderRadius: 6,
            padding: "12px 14px",
            fontSize: 11,
            lineHeight: 1.5,
            maxHeight: 360,
            overflow: "auto",
            marginBottom: 16,
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
            fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
            color: T.text,
          }}
        >
          {state.specMarkdown}
        </pre>
      )}

      <div
        style={{
          background: T.warnSoft,
          color: T.warn,
          border: `1px solid ${T.warn}`,
          borderRadius: 6,
          padding: "10px 14px",
          fontSize: 12,
          marginBottom: 16,
        }}
      >
        <strong>Hard gates before merge</strong>: cargo build ✓ &nbsp;·&nbsp; ≥1
        grpcurl Authorize call → 2xx ✓ &nbsp;·&nbsp; no PAYMENT_FLOW_ERROR ✓
        &nbsp;·&nbsp; never retry without a code change. Pipeline enforces these
        in <code>compiler</code> and <code>grpc_test</code> checkpoints.
      </div>

      <ReviewBlock
        title="1. Basics"
        onEdit={() => dispatch({ type: "goto", step: 1 })}
        rows={[
          ["Connector", state.connectorName],
          ["Display name", state.displayName || "—"],
          ["Description", trunc(state.description, 120)],
          ["OpenAPI URL", state.openApiUrl || "—"],
          ["Postman URL", state.postmanCollectionUrl || "—"],
          ["Integration guide", state.integrationGuideUrl || "—"],
          ["Additional docs", String(state.docs.length)],
        ]}
      />

      <ReviewBlock
        title="2. Technical"
        onEdit={() => dispatch({ type: "goto", step: 2 })}
        rows={[
          ["Auth scheme", state.authScheme || "—"],
          ["Auth location", state.authLocation || "—"],
          ["Credential fields", state.credentialFields.join(", ") || "—"],
          ["Currency unit", state.currencyUnit || "—"],
          ["Base URL", state.baseUrl || "—"],
          ["Sandbox URL", state.sandboxUrl || "—"],
          [
            "Feature toggles",
            [
              state.supports3DS && "3DS",
              state.supportsWebhooks && "Webhooks",
              state.supportsRecurring && "Recurring",
            ]
              .filter(Boolean)
              .join(", ") || "—",
          ],
          ["Flows", state.supportedFlows.join(", ")],
          ["Payment methods", pms.join(", ") || "—"],
          ["Primary PM", state.primaryPaymentMethod || "—"],
          ["Webhook pattern", state.webhookUrlPattern || "—"],
          ["Regions", state.regions.join(", ") || "—"],
          ["Currencies", state.supportedCurrencies || "—"],
        ]}
      />

      <ReviewBlock
        title="3. Acceptance & Runner"
        onEdit={() => dispatch({ type: "goto", step: 3 })}
        rows={[
          [
            "Acceptance criteria",
            state.acceptanceCriteria
              .filter((c) => c.trim())
              .map((c, i) => `${i + 1}. ${c}`)
              .join("\n") || "—",
          ],
          ["Prerequisites", state.prerequisites.join(", ") || "—"],
          ["Complexity", state.estimatedComplexity],
          ["Priority", state.priority],
          ["Notes", trunc(state.connectorNotes, 120) || "—"],
          ["Runner", state.runner || "—"],
          ["Model override", state.runnerModel || "(config default)"],
          ["Source path", state.sourcePath],
          ["Copy strategy", state.copyStrategy],
        ]}
      />

      <div
        style={{
          marginTop: 16,
          padding: "12px 14px",
          background: T.bgElev,
          border: `1px solid ${T.border}`,
          borderRadius: 6,
          fontSize: 12,
          color: T.textMuted,
        }}
      >
        On launch, the pipeline runs: <strong>task</strong> → <strong>preflight</strong> →{" "}
        <strong>requirements</strong> → <strong>l2_planning</strong> →{" "}
        <strong>l2_review</strong> → <strong>l3_analysis</strong> →{" "}
        <strong>l3_review</strong> → <strong>implementation</strong> →{" "}
        <strong>compiler</strong> → <strong>grpc_test</strong> →{" "}
        <strong>pr_review</strong>. You'll be auto-routed to the session page to
        watch it run.
      </div>
    </div>
  );
}

function ReviewBlock({
  title,
  rows,
  onEdit,
}: {
  title: string;
  rows: [string, string][];
  onEdit: () => void;
}) {
  return (
    <div
      style={{
        marginBottom: 14,
        padding: 16,
        background: T.bgElev,
        border: `1px solid ${T.border}`,
        borderRadius: 8,
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 10,
        }}
      >
        <h4
          style={{
            fontSize: 13,
            fontWeight: 700,
            color: T.textMuted,
            textTransform: "uppercase",
            letterSpacing: 1,
            margin: 0,
          }}
        >
          {title}
        </h4>
        <button
          type="button"
          onClick={onEdit}
          style={{
            background: "transparent",
            border: "none",
            color: T.accent,
            fontSize: 12,
            fontWeight: 600,
            cursor: "pointer",
          }}
        >
          Edit
        </button>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "180px 1fr", rowGap: 6 }}>
        {rows.map(([k, v]) => (
          <div key={k} style={{ display: "contents" }}>
            <div style={{ fontSize: 12, color: T.textSubtle }}>{k}</div>
            <div
              style={{
                fontSize: 13,
                color: T.text,
                whiteSpace: "pre-wrap",
                wordBreak: "break-word",
              }}
            >
              {v}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function trunc(s: string, n: number): string {
  if (!s) return "";
  return s.length > n ? s.slice(0, n) + "…" : s;
}
