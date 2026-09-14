/**
 * Structured progress emitter for the autopilot heartbeat.
 *
 * Each event is written to stdout as a single line prefixed with
 * `__PARITY_PROGRESS__ ` so a downstream bridge (e.g. the dashboard's
 * Vite SSE plugin) can filter structured events out of the free-form
 * agent + cargo log noise.
 *
 * Free-form `console.log` from agents stays as-is — humans reading the
 * terminal see everything; machine consumers grep for the marker.
 */

export type ProgressEvent =
  | { phase: "discover"; status: "start" | "ok" | "fail"; detail?: string; leafCount?: number }
  | { phase: "refresh"; status: "ok" }
  | { phase: "sweep"; status: "ok" }
  | { phase: "decide"; status: "ok" | "fail"; picked?: number; reason?: string }
  | { phase: "claim"; status: "start" | "skipped-dry-run" | "ok" | "raced" | "fail"; detail?: string }
  | { phase: "understand"; status: "start" | "ok" | "fail"; confidence?: string; locus?: string; markdown?: string }
  | { phase: "plan"; status: "start" | "ok" | "fail"; markdown?: string }
  | { phase: "execute"; status: "start" | "ok" | "fail"; target?: string; branch?: string; tail?: string }
  | { phase: "verify"; status: "start" | "ok" | "fail"; markdown?: string; reason?: string }
  | { phase: "handoff"; status: "start" | "skipped-dry-run" | "ok" | "fail"; prUrl?: string }
  | { phase: "escalate"; status: "ok"; step: string; blocker: string }
  | { phase: "done"; outcome: string; detail?: string; pickedLeaf?: number };

export const PARITY_PROGRESS_MARKER = "__PARITY_PROGRESS__";

export function emit(ev: ProgressEvent): void {
  try {
    process.stdout.write(`${PARITY_PROGRESS_MARKER} ${JSON.stringify(ev)}\n`);
  } catch {
    // stdout closed — orchestrator survives without progress emission
  }
}
