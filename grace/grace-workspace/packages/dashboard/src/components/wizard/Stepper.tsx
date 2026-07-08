import { T } from "../../theme";

const STEPS = [
  { n: 1, label: "Basics" },
  { n: 2, label: "Technical" },
  { n: 3, label: "Acceptance" },
  { n: 4, label: "Review" },
];

export function Stepper({
  current,
  reached,
  onJump,
}: {
  current: number;
  reached: number; // furthest step the user has validated past
  onJump: (step: number) => void;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 0,
        padding: "16px 24px",
        background: T.bg,
        borderBottom: `1px solid ${T.border}`,
      }}
    >
      {STEPS.map((s, i) => {
        const isCurrent = s.n === current;
        const isReached = s.n <= reached;
        const canClick = isReached && !isCurrent;
        return (
          <div
            key={s.n}
            style={{ display: "flex", alignItems: "center", flex: i < STEPS.length - 1 ? 1 : 0 }}
          >
            <button
              type="button"
              onClick={() => canClick && onJump(s.n)}
              disabled={!canClick}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                background: "transparent",
                border: "none",
                cursor: canClick ? "pointer" : "default",
                padding: 0,
                outline: "none",
              }}
            >
              <span
                style={{
                  width: 28,
                  height: 28,
                  borderRadius: "50%",
                  border: `2px solid ${isCurrent ? T.accent : isReached ? T.accent : T.border}`,
                  background: isCurrent
                    ? T.accent
                    : isReached
                    ? T.accentSoft
                    : T.bg,
                  color: isCurrent ? "#fff" : isReached ? T.accent : T.textSubtle,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  fontSize: 13,
                  fontWeight: 700,
                }}
              >
                {s.n}
              </span>
              <span
                style={{
                  fontSize: 13,
                  fontWeight: isCurrent ? 700 : 500,
                  color: isCurrent ? T.text : isReached ? T.textMuted : T.textSubtle,
                }}
              >
                {s.label}
              </span>
            </button>
            {i < STEPS.length - 1 && (
              <div
                style={{
                  flex: 1,
                  height: 2,
                  margin: "0 12px",
                  background: s.n < reached ? T.accent : T.border,
                }}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}
