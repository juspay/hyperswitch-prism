import { useState } from "react";
import { T } from "../theme";

export function DesignGatePrompt({
  currentFigmaUrl,
  onRespond,
}: {
  currentFigmaUrl?: string;
  onRespond: (payload: {
    designRequired: boolean;
    figmaUrl?: string;
    skipReason?: string;
  }) => void;
}) {
  const [mode, setMode] = useState<"idle" | "yes" | "no">("idle");
  const [figmaUrl, setFigmaUrl] = useState(currentFigmaUrl ?? "");
  const [skipReason, setSkipReason] = useState("");
  const [error, setError] = useState<string | null>(null);

  const submitYes = () => {
    if (!/^https?:\/\/(www\.)?figma\.com\//.test(figmaUrl.trim())) {
      setError("Please paste a valid figma.com URL");
      return;
    }
    onRespond({ designRequired: true, figmaUrl: figmaUrl.trim() });
  };

  const submitNo = () => {
    onRespond({ designRequired: false, skipReason: skipReason.trim() || undefined });
  };

  return (
    <div
      style={{
        background: T.warnSoft,
        border: `1px solid ${T.warn}`,
        borderRadius: 12,
        padding: 22,
        marginBottom: 20,
        maxWidth: 640,
        boxShadow: T.shadow,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 14 }}>
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
          Does this task require UI design?
        </div>
      </div>

      {mode === "idle" && (
        <div>
          <div style={{ fontSize: 13, color: T.textMuted, marginBottom: 14 }}>
            If yes, provide a Figma URL. If no, the visual-diff checkpoint will be
            skipped.
          </div>
          <div style={{ display: "flex", gap: 10 }}>
            <button onClick={() => setMode("yes")} style={btn(T.accent, "#fff")}>
              ✎ Yes — needs design
            </button>
            <button
              onClick={() => setMode("no")}
              style={btn(T.bgElev, T.text, T.borderStrong)}
            >
              ✗ No — skip design
            </button>
          </div>
        </div>
      )}

      {mode === "yes" && (
        <div>
          <label style={labelStyle}>Figma URL</label>
          <input
            value={figmaUrl}
            onChange={(e) => {
              setFigmaUrl(e.target.value);
              setError(null);
            }}
            placeholder="https://www.figma.com/design/..."
            style={inputStyle}
          />
          {error && (
            <div style={{ color: T.error, fontSize: 12, marginTop: 6 }}>{error}</div>
          )}
          <div style={{ display: "flex", gap: 10, marginTop: 12 }}>
            <button onClick={submitYes} style={btn(T.success, "#fff")}>
              Submit
            </button>
            <button
              onClick={() => setMode("idle")}
              style={btn(T.bgElev, T.text, T.borderStrong)}
            >
              Back
            </button>
          </div>
        </div>
      )}

      {mode === "no" && (
        <div>
          <label style={labelStyle}>Reason (optional)</label>
          <input
            value={skipReason}
            onChange={(e) => setSkipReason(e.target.value)}
            placeholder="e.g. text-only change, no visual output"
            style={inputStyle}
          />
          <div style={{ display: "flex", gap: 10, marginTop: 12 }}>
            <button onClick={submitNo} style={btn(T.success, "#fff")}>
              Skip design stage
            </button>
            <button
              onClick={() => setMode("idle")}
              style={btn(T.bgElev, T.text, T.borderStrong)}
            >
              Back
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

const labelStyle: React.CSSProperties = {
  display: "block",
  fontSize: 11,
  fontWeight: 700,
  color: T.textMuted,
  textTransform: "uppercase",
  marginBottom: 6,
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "9px 12px",
  border: `1px solid ${T.border}`,
  borderRadius: 6,
  background: T.bgElev,
  color: T.text,
  fontSize: 13,
  fontFamily: "inherit",
  boxSizing: "border-box",
};

function btn(
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
