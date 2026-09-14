import { useState } from "react";
import { Link } from "react-router-dom";
import { T } from "../theme";
import type { BoardCard, BoardStep } from "../hooks/usePrResolver";

const STATUS_DOT: Record<BoardCard["status"], string> = {
  queued: "#9ca3af",
  in_progress: "#3b82f6",
  completed: T.success,
  failed: T.error,
};

const STATUS_BG: Record<BoardCard["status"], string> = {
  queued: T.bg,
  in_progress: T.bgElev,
  completed: T.successSoft,
  failed: T.errorSoft,
};

interface Props {
  card: BoardCard;
}

export function PrResolverCard({ card }: Props) {
  const [expanded, setExpanded] = useState(false);
  const steps = card.steps;
  const visibleSteps = expanded ? steps : steps.slice(-3);
  const dotColor = STATUS_DOT[card.status];
  const bg = STATUS_BG[card.status];

  return (
    <Link
      to={`/pr-resolver/${card.pr}`}
      style={{
        background: bg,
        border: `1px solid ${T.border}`,
        borderRadius: 8,
        padding: 12,
        boxShadow: T.shadow,
        cursor: "pointer",
        textDecoration: "none",
        color: "inherit",
        display: "block",
      }}
      onClick={(e) => {
        // Tap the chevron at the bottom of the card to expand inline without
        // navigating; everywhere else navigates to the detail page.
        const target = e.target as HTMLElement;
        if (target.dataset.cardAction === "expand") {
          e.preventDefault();
          setExpanded((v) => !v);
        }
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 6,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span
            style={{
              width: 8,
              height: 8,
              borderRadius: "50%",
              background: dotColor,
              display: "inline-block",
              animation:
                card.status === "in_progress"
                  ? "prResolverPulse 1.5s infinite"
                  : undefined,
            }}
          />
          <span style={{ fontSize: 13, fontWeight: 600, color: T.text }}>
            {card.title}
          </span>
          {card.connector && (
            <span
              style={{
                fontSize: 10,
                color: T.textMuted,
                background: T.codeBg,
                padding: "2px 6px",
                borderRadius: 4,
                fontFamily: "monospace",
              }}
            >
              {card.connector}
            </span>
          )}
        </div>
        {card.commitSha && (
          <code
            style={{
              fontSize: 10,
              color: T.textMuted,
              background: T.codeBg,
              padding: "2px 6px",
              borderRadius: 4,
            }}
          >
            {card.commitSha.slice(0, 8)}
          </code>
        )}
      </div>

      {card.detail && (
        <div
          style={{
            fontSize: 11,
            color: T.textMuted,
            marginBottom: steps.length > 0 ? 8 : 0,
            lineHeight: 1.4,
          }}
        >
          {card.detail}
        </div>
      )}

      {card.path && (
        <div
          style={{
            fontSize: 10,
            color: T.textSubtle,
            fontFamily: "monospace",
            marginBottom: steps.length > 0 ? 8 : 0,
          }}
        >
          {card.path}
        </div>
      )}

      {card.error && (
        <div
          style={{
            fontSize: 11,
            color: T.error,
            background: T.errorSoft,
            border: `1px solid ${T.error}33`,
            padding: 6,
            borderRadius: 4,
            marginBottom: steps.length > 0 ? 8 : 0,
            fontFamily: "monospace",
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
          }}
        >
          {card.error.slice(0, 300)}
        </div>
      )}

      {visibleSteps.length > 0 && (
        <ul
          style={{
            margin: 0,
            padding: "8px 0 0 16px",
            borderTop: `1px dashed ${T.border}`,
            listStyle: "none",
            display: "flex",
            flexDirection: "column",
            gap: 4,
          }}
        >
          {visibleSteps.map((step, i) => (
            <StepRow key={i} step={step} />
          ))}
        </ul>
      )}

      {steps.length > 3 && (
        <div
          data-card-action="expand"
          style={{
            fontSize: 10,
            color: T.textSubtle,
            textAlign: "right",
            marginTop: 4,
            cursor: "pointer",
          }}
        >
          {expanded
            ? `▲ collapse`
            : `▾ ${steps.length - visibleSteps.length} more step(s)`}
        </div>
      )}
      <div
        style={{
          fontSize: 10,
          color: T.accent,
          marginTop: 6,
          textAlign: "right",
          fontWeight: 600,
        }}
      >
        open detail →
      </div>
    </Link>
  );
}

function StepRow({ step }: { step: BoardStep }) {
  const icon =
    step.passed === true ? "✓" : step.passed === false ? "✕" : "•";
  const iconColor =
    step.passed === true
      ? T.success
      : step.passed === false
        ? T.error
        : T.textMuted;
  return (
    <li
      style={{
        fontSize: 11,
        color: T.text,
        display: "flex",
        gap: 6,
        alignItems: "flex-start",
      }}
    >
      <span style={{ color: iconColor, fontWeight: 700, lineHeight: "16px" }}>
        {icon}
      </span>
      <span style={{ flex: 1, lineHeight: 1.4 }}>
        {step.text}
        {step.detail && (
          <span
            style={{
              color: T.textSubtle,
              marginLeft: 4,
              fontFamily: "monospace",
            }}
          >
            {" — "}
            {step.detail.slice(0, 160)}
          </span>
        )}
      </span>
    </li>
  );
}
