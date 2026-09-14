import { useEffect, useRef, useState } from "react";
import { T } from "../theme";

const MESSAGES: Record<string, string[]> = {
  product_alignment: [
    "PM is reading the ticket very carefully…",
    "PM: 'is this a P0 or a P1?' 🤔",
    "PM: 'did we socialize this with stakeholders?'",
    "PM opens Notion. PM closes Notion. PM opens Notion again.",
    "PM: 'so what's the success metric here?' 📊",
    "PM: 'we'll loop in design async'",
    "PM: 'I love this, just one small thing…'  (it's never small)",
    "PM checking if this conflicts with the Q2 roadmap…",
    "PM: 'can we scope this down to an MVP?'",
    "PM: 'great idea, let's talk offline' 🧊",
    "PM drafting a doc about the doc about the doc…",
    "PM: 'have we talked to customer success about this?'",
    "PM adding a row to the prioritization sheet…",
    "PM: 'just to play devil's advocate…'",
    "PM: 'what's the blast radius if this breaks?' 💥",
    "PM: 'circle back after sprint planning' 🔁",
  ],
  l2_planning: [
    "Discovering connector API documentation…",
    "Reading payment method integration guides…",
    "Analyzing authentication schemes…",
    "Compiling technical specification…",
    "Senior eng researching connector patterns…",
    "Eng: 'what are the request/response schemas?'",
    "Eng analyzing error handling patterns…",
    "Eng documenting webhook requirements…",
    "Eng writing comprehensive tech spec…",
    "Validating documentation URLs…",
    "Structuring 8-section specification…",
  ],
  l3_analysis: [
    "Analyzing 6 reference files from 2.3_codegen.md…",
    "Reading tech spec and pattern guides…",
    "Understanding macro patterns…",
    "Analyzing domain types…",
    "Reviewing existing connector code…",
    "Checking transformers implementation…",
    "Identifying patterns and conventions…",
    "Listing flows in create_all_prerequisites!…",
    "Verifying prerequisites status…",
    "Documenting implementation approach…",
  ],
  implementation: [
    "Writing actual ReScript code 🐫",
    "Eng executing the approved plan, no improvising…",
    "Eng typing `React.element` for the 47th time today…",
    "Eng: 'the plan said modify, so we modify'",
    "Eng resisting the urge to write `Obj.magic`…",
    "Eng: 'is this over-engineered? yes. shipping anyway.' 🚢",
  ],
  compiler: [
    "Running `rescript build`…",
    "Compiler: 'type mismatch' (helpful!)",
    "Hunting down a missing semicolon in a language without semicolons…",
    "Hoping the types line up this time…",
    "Praying to the ReScript gods 🙏",
    "Compiler found something. Is it your fault? Probably.",
  ],
  cypress: [
    "Cypress warming up the browser…",
    "Clicking buttons very fast 🖱️",
    "Finding out which tests are actually flaky…",
    "`cy.wait(500)` is the only thing holding this together…",
  ],
  playwright: [
    "Playwright launching three browsers at once 🕹️",
    "Chrome, Firefox, and Safari walk into a bar…",
    "Cross-browser: Safari disagrees. Naturally.",
  ],
  pr_review: [
    "Senior eng reading your diff with narrowed eyes 👀",
    "Reviewer: 'could we pull this into a helper?'",
    "Reviewer: 'nit: rename this' (there are 14 nits)",
    "Reviewer: 'have you considered… (long paragraph)'",
    "Reviewer: 'lgtm with small comments' — half the PR rewritten",
    "Checking spec compliance line by line…",
  ],
  regression: [
    "Running the full test suite…",
    "Waiting for CI like it's 1999…",
    "Regression: catching the bug you fixed last sprint reappearing 🐛",
  ],
  default: [
    "Thinking…",
    "Working on it…",
    "Almost there…",
    "Typing furiously…",
  ],
};

const DURATION_MESSAGES = [
  "Still chewing on it…",
  "This one's a ticket-and-a-half 🎫",
  "LLMs need their thinking time 🧠",
  "Grab a coffee ☕ — we'll be here a sec",
  "PM just asked one more follow-up question…",
  "Standup is in 10, we gotta hurry…",
  "Sprint ends Friday, no pressure 🙃",
];

export function LoadingState({
  checkpointId,
  logs,
}: {
  checkpointId: string;
  /**
   * Last N log lines for this checkpoint. When non-empty, the spinner card
   * renders a tailing log panel below the rotating-joke text so the user
   * sees what the agent is doing (Read, Edit, Bash, etc.) instead of a
   * dumb spinner. Lines are passed pre-filtered by CheckpointDetail.
   */
  logs?: string[];
}) {
  const pool = MESSAGES[checkpointId] ?? MESSAGES.default!;
  const [idx, setIdx] = useState(() => Math.floor(Math.random() * pool.length));
  const [elapsed, setElapsed] = useState(0);
  const logRef = useRef<HTMLDivElement>(null);

  // Auto-scroll the log tail to the bottom whenever a new line arrives.
  useEffect(() => {
    if (logRef.current) {
      logRef.current.scrollTop = logRef.current.scrollHeight;
    }
  }, [logs?.length]);

  useEffect(() => {
    const t = setInterval(() => setIdx((i) => (i + 1) % pool.length), 2600);
    return () => clearInterval(t);
  }, [pool.length]);

  useEffect(() => {
    const start = Date.now();
    const t = setInterval(() => setElapsed(Math.floor((Date.now() - start) / 1000)), 1000);
    return () => clearInterval(t);
  }, [checkpointId]);

  const longMessage =
    elapsed > 15
      ? DURATION_MESSAGES[Math.floor(elapsed / 15) % DURATION_MESSAGES.length]
      : null;

  return (
    <div
      style={{
        maxWidth: 560,
        padding: "36px 32px",
        background: T.bgElev,
        border: `1px solid ${T.border}`,
        borderRadius: 14,
        boxShadow: T.shadow,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 18,
      }}
    >
      {/* Spinner */}
      <div
        style={{
          position: "relative",
          width: 56,
          height: 56,
        }}
      >
        <div
          style={{
            position: "absolute",
            inset: 0,
            borderRadius: "50%",
            border: `3px solid ${T.accentSoft}`,
          }}
        />
        <div
          style={{
            position: "absolute",
            inset: 0,
            borderRadius: "50%",
            border: `3px solid transparent`,
            borderTopColor: T.accent,
            borderRightColor: T.accent,
            animation: "spin 0.9s linear infinite",
          }}
        />
        <div
          style={{
            position: "absolute",
            inset: 16,
            borderRadius: "50%",
            background: T.accent,
            animation: "pulse 1.4s ease-in-out infinite",
          }}
        />
      </div>

      {/* Quirky rotating message */}
      <div
        key={idx}
        style={{
          fontSize: 15,
          fontWeight: 600,
          color: T.text,
          textAlign: "center",
          minHeight: 22,
          animation: "fade 0.4s ease-out",
        }}
      >
        {pool[idx]}
      </div>

      {longMessage && (
        <div
          style={{
            fontSize: 12,
            color: T.textMuted,
            fontStyle: "italic",
            textAlign: "center",
          }}
        >
          {longMessage}
        </div>
      )}

      {/* Elapsed timer */}
      <div
        style={{
          fontSize: 11,
          color: T.textSubtle,
          fontVariantNumeric: "tabular-nums",
          letterSpacing: 0.5,
        }}
      >
        {elapsed}s elapsed
      </div>

      {/* Live agent log tail. Populated for any checkpoint whose runner
          streams via `onStdoutLine` (currently implementation + per-flow
          subagents + scaffold). When empty we fall back to just the
          rotating-joke text above. */}
      {logs && logs.length > 0 && (
        <div
          ref={logRef}
          style={{
            width: "100%",
            maxHeight: 220,
            overflowY: "auto",
            background: T.codeBg,
            border: `1px solid ${T.border}`,
            borderRadius: 8,
            padding: "10px 12px",
            fontFamily:
              "ui-monospace, SFMono-Regular, Menlo, Monaco, 'Cascadia Code', 'Source Code Pro', monospace",
            fontSize: 11.5,
            lineHeight: 1.45,
            color: T.text,
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
            textAlign: "left",
          }}
        >
          {logs.map((line, i) => (
            <div
              key={i}
              style={{
                color: line.startsWith("✗") || line.startsWith("   ✗")
                  ? T.error
                  : line.startsWith("✓") || line.startsWith("   ✓")
                    ? T.success
                    : line.startsWith("🔧")
                      ? T.accent
                      : T.text,
              }}
            >
              {line}
            </div>
          ))}
        </div>
      )}

      <style>{`
        @keyframes fade { from { opacity: 0; transform: translateY(4px); } to { opacity: 1; transform: none; } }
      `}</style>
    </div>
  );
}
