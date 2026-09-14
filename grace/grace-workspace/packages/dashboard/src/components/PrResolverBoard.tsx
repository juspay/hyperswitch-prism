import { T } from "../theme";
import { PrResolverCard } from "./PrResolverCard";
import type { BoardCard, PrResolverBoardState } from "../hooks/usePrResolver";

const COLUMN_DEFS: Array<{
  key: keyof PrResolverBoardState;
  label: string;
  dot: string;
  pulse?: boolean;
}> = [
  { key: "queued", label: "Queued", dot: "#9ca3af" },
  { key: "inProgress", label: "In Progress", dot: "#3b82f6", pulse: true },
  { key: "completed", label: "Completed", dot: T.success },
  { key: "failed", label: "Failed / Blocked", dot: T.error },
];

interface Props {
  board: PrResolverBoardState;
}

export function PrResolverBoard({ board }: Props) {
  return (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: "repeat(4, minmax(0, 1fr))",
        gap: 16,
        padding: 24,
      }}
    >
      {COLUMN_DEFS.map((col) => {
        const items = board[col.key] as BoardCard[];
        return (
          <div key={col.key} style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            <ColumnHeader
              label={col.label}
              count={items.length}
              dot={col.dot}
              pulse={col.pulse}
            />
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                gap: 10,
                minHeight: 400,
              }}
            >
              {items.length === 0 ? (
                <EmptyColumn />
              ) : (
                items.map((card) => <PrResolverCard key={card.key} card={card} />)
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}

function ColumnHeader({
  label,
  count,
  dot,
  pulse,
}: {
  label: string;
  count: number;
  dot: string;
  pulse?: boolean;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        paddingBottom: 8,
        borderBottom: `1px solid ${T.border}`,
      }}
    >
      <span
        style={{
          width: 10,
          height: 10,
          borderRadius: "50%",
          background: dot,
          display: "inline-block",
          animation: pulse ? "prResolverPulse 1.5s infinite" : undefined,
        }}
      />
      <span style={{ fontSize: 13, fontWeight: 600, color: T.text }}>
        {label}
      </span>
      <span
        style={{
          fontSize: 11,
          color: T.textMuted,
          background: T.bgElev,
          padding: "2px 8px",
          borderRadius: 10,
          marginLeft: "auto",
        }}
      >
        {count}
      </span>
    </div>
  );
}

function EmptyColumn() {
  return (
    <div
      style={{
        fontSize: 12,
        color: T.textSubtle,
        fontStyle: "italic",
        padding: 12,
        textAlign: "center",
        border: `1px dashed ${T.border}`,
        borderRadius: 6,
        background: T.bg,
      }}
    >
      empty
    </div>
  );
}
