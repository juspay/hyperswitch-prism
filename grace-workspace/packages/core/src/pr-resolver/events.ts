import { EventEmitter } from "node:events";

/**
 * Event types the resolver service emits over the lifetime of a poll cycle.
 * The supervisor subscribes to these and broadcasts each one to dashboards
 * as `pr-resolver:<type>`. Standalone CLI runs (`byne pr-resolver --once`)
 * subscribe and log instead — keeps the service decoupled from how events
 * are delivered.
 */
export type PrResolverEventType =
  | "cycle_start"
  | "cycle_end"
  | "cycle_skipped_pending_approval"
  | "no_comments"
  | "awaiting_approval"
  | "approved"
  | "rejected"
  | "revision_requested"
  | "revision_applying"
  | "pr_retry"
  | "machine_changed"
  | "comment_found"
  | "comment_reacted"
  | "comment_unauthorized"
  | "pr_start"
  | "pr_queued"
  | "pr_done"
  | "pr_skipped"
  | "pr_failed"
  | "gate"
  | "subtask_start"
  | "subtask_gate"
  | "subtask_done"
  | "subtask_failed"
  | "clone_acquired"
  | "checkout_done"
  | "resolver_text"
  | "resolver_tool"
  | "resolver_stream"
  | "resolver_done"
  | "build_start"
  | "build_pass"
  | "build_fail"
  | "clippy_pass"
  | "clippy_fail"
  | "grpc_test_extracted"
  | "grpc_test_generated"
  | "grpc_test_plan_generated"
  | "grpc_test_plan_parse_error"
  | "grpc_test_step_start"
  | "grpc_test_step_pass"
  | "grpc_test_step_fail"
  | "grpc_test_step_skipped"
  | "grpc_server_starting"
  | "grpc_server_log"
  | "grpc_server_probe"
  | "grpc_server_ready"
  | "grpc_server_stopped"
  | "grpc_test_command"
  | "grpc_test_pass"
  | "grpc_test_fail"
  | "grpc_test_skipped"
  | "push_done"
  | "reply_posted"
  | "error"
  | "state_changed";

export interface PrResolverEvent {
  type: PrResolverEventType;
  timestamp: number;
  payload: Record<string, unknown>;
}

/** Cap on the in-memory replay buffer for late-joining dashboards. */
const MAX_RECENT = 500;

/**
 * Events excluded from the replay buffer. `resolver_stream` fires once per
 * line of `claude --verbose` stdout — hundreds per cycle — and only matters
 * in real-time. Keeping them out of the buffer leaves room for the state-
 * shaping events (pr_start / pr_done / push_done) that the dashboard needs
 * to reconstruct correct card statuses on reconnect.
 */
const NON_REPLAY_TYPES: ReadonlySet<PrResolverEventType> = new Set([
  "resolver_stream",
  "grpc_server_log",
  "grpc_server_probe",
]);

const emitter = new EventEmitter();
emitter.setMaxListeners(50);
const recent: PrResolverEvent[] = [];

export function emitPrResolverEvent(
  type: PrResolverEventType,
  payload: Record<string, unknown> = {}
): void {
  const event: PrResolverEvent = {
    type,
    timestamp: Date.now(),
    payload,
  };
  if (!NON_REPLAY_TYPES.has(type)) {
    recent.push(event);
    if (recent.length > MAX_RECENT) recent.shift();
  }
  emitter.emit("event", event);
}

export function onPrResolverEvent(
  listener: (event: PrResolverEvent) => void
): () => void {
  emitter.on("event", listener);
  return () => emitter.off("event", listener);
}

/**
 * Snapshot of the last N events. Used by the supervisor when a fresh
 * dashboard tab connects so the user doesn't see a blank Kanban while
 * waiting for the next event.
 */
export function getRecentPrResolverEvents(limit = 200): PrResolverEvent[] {
  if (limit >= recent.length) return [...recent];
  return recent.slice(-limit);
}
