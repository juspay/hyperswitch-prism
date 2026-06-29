import { useState } from "react";
import { T } from "../theme";
import { ArtifactView } from "./ArtifactView";

export function HumanReview({
  checkpointId,
  spec,
  onRespond,
  rejectionReason,
}: {
  checkpointId: string;
  spec: unknown;
  onRespond: (payload: {
    decision: "approve" | "edit" | "regenerate";
    editedSpec?: unknown;
    regeneratePrompt?: string;
    notes?: string;
  }) => void;
  rejectionReason?: string | null;
}) {
  const [mode, setMode] = useState<"idle" | "regenerate" | "edit">(
    rejectionReason ? "edit" : "idle"
  );
  const [guidance, setGuidance] = useState("");
  const [editedJson, setEditedJson] = useState(() =>
    JSON.stringify(spec, null, 2)
  );
  const [editError, setEditError] = useState<string | null>(null);

  // Map checkpointId → artifact render key
  const artifactRenderId: Record<string, string> = {
    l2_review: "l2_planning",
    l3_review: "l3_analysis",
  };

  return (
    <div
      style={{
        background: T.warnSoft,
        border: `1px solid ${T.warn}`,
        borderRadius: 12,
        padding: 22,
        marginBottom: 20,
        maxWidth: 760,
        boxShadow: T.shadow,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 10,
          marginBottom: 14,
        }}
      >
        <div
          style={{
            width: 10,
            height: 10,
            borderRadius: "50%",
            background: T.warn,
            animation: "pulse 1.6s ease-in-out infinite",
          }}
        />
        <div style={{ fontWeight: 700, color: T.warn, fontSize: 14 }}>
          Awaiting your review
        </div>
      </div>
      <div style={{ fontSize: 12, color: T.textMuted, marginBottom: 14 }}>
        The pipeline is paused. Review the generated spec below and choose
        Approve, Edit, or Regenerate. The engine is waiting for your response.
      </div>

      <div style={{ marginBottom: 16 }}>
        <ArtifactView
          checkpointId={artifactRenderId[checkpointId] ?? checkpointId}
          artifact={spec}
        />
      </div>

      {mode === "idle" && (
        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
          <button
            onClick={() => onRespond({ decision: "approve" })}
            style={btnStyle(T.success, "#fff")}
          >
            ✓ Approve
          </button>
          <button
            onClick={() => setMode("edit")}
            style={btnStyle(T.bgElev, T.text, T.borderStrong)}
          >
            ✎ Edit spec
          </button>
          <button
            onClick={() => setMode("regenerate")}
            style={btnStyle(T.accent, "#fff")}
          >
            ↻ Regenerate
          </button>
        </div>
      )}

      {mode === "regenerate" && (
        <div>
          <div
            style={{
              fontSize: 11,
              fontWeight: 700,
              color: T.textMuted,
              textTransform: "uppercase",
              marginBottom: 6,
            }}
          >
            How should the regenerated spec differ?
          </div>
          <textarea
            value={guidance}
            onChange={(e) => setGuidance(e.target.value)}
            placeholder="e.g. Split the auth task into separate login and logout. Add loading state handling."
            style={{
              width: "100%",
              minHeight: 90,
              padding: 10,
              border: `1px solid ${T.border}`,
              borderRadius: 6,
              background: T.bgElev,
              color: T.text,
              fontFamily: "inherit",
              fontSize: 13,
              resize: "vertical",
              boxSizing: "border-box",
            }}
          />
          <div style={{ display: "flex", gap: 10, marginTop: 10 }}>
            <button
              onClick={() => {
                if (guidance.trim().length < 5) return;
                onRespond({
                  decision: "regenerate",
                  regeneratePrompt: guidance.trim(),
                });
              }}
              disabled={guidance.trim().length < 5}
              style={btnStyle(
                guidance.trim().length >= 5 ? T.accent : T.border,
                guidance.trim().length >= 5 ? "#fff" : T.textSubtle
              )}
            >
              Submit regeneration request
            </button>
            <button
              onClick={() => setMode("idle")}
              style={btnStyle(T.bgElev, T.text, T.borderStrong)}
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {mode === "edit" && (
        <div>
          <div
            style={{
              fontSize: 11,
              fontWeight: 700,
              color: T.textMuted,
              textTransform: "uppercase",
              marginBottom: 6,
            }}
          >
            Edit the raw spec JSON — your edits are final, no re-generation
          </div>
          {rejectionReason && (
            <div
              style={{
                marginBottom: 10,
                padding: "9px 12px",
                background: T.errorSoft,
                border: `1px solid ${T.error}`,
                borderRadius: 6,
                color: T.error,
                fontSize: 12,
              }}
            >
              <strong>Previous submission rejected:</strong> {rejectionReason}
            </div>
          )}
          <textarea
            value={editedJson}
            onChange={(e) => {
              setEditedJson(e.target.value);
              setEditError(null);
            }}
            style={{
              width: "100%",
              minHeight: 260,
              padding: 10,
              border: `1px solid ${editError ? T.error : T.border}`,
              borderRadius: 6,
              background: T.codeBg,
              color: T.text,
              fontFamily: "ui-monospace, SFMono-Regular, monospace",
              fontSize: 12,
              resize: "vertical",
              boxSizing: "border-box",
            }}
          />
          {editError && (
            <div
              style={{
                color: T.error,
                fontSize: 12,
                marginTop: 6,
              }}
            >
              {editError}
            </div>
          )}
          <div style={{ display: "flex", gap: 10, marginTop: 10 }}>
            <button
              onClick={() => {
                try {
                  const parsed = JSON.parse(editedJson);
                  onRespond({ decision: "edit", editedSpec: parsed });
                } catch (err) {
                  setEditError(
                    `Invalid JSON: ${err instanceof Error ? err.message : String(err)}`
                  );
                }
              }}
              style={btnStyle(T.success, "#fff")}
            >
              Save edited spec
            </button>
            <button
              onClick={() => setMode("idle")}
              style={btnStyle(T.bgElev, T.text, T.borderStrong)}
            >
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function btnStyle(
  bg: string,
  fg: string,
  borderColor?: string
): React.CSSProperties {
  return {
    padding: "9px 16px",
    borderRadius: 6,
    border: `1px solid ${borderColor ?? bg}`,
    background: bg,
    color: fg,
    fontSize: 13,
    fontWeight: 600,
    cursor: "pointer",
  };
}
