/**
 * Shared data types for the PR Resolver. Mirrors the Python `pr_resolver`
 * dataclasses (github.py + resolver.py + service.py) but camel-cased and
 * TypeScript-friendly.
 */

export interface ReviewComment {
  id: string;
  body: string;
  author: string;
  authorAssociation: string;
  createdAt: string;
  updatedAt: string;
  diffHunk: string;
}

export interface ReviewThread {
  id: string;
  isResolved: boolean;
  isOutdated: boolean;
  path: string;
  line: number | null;
  startLine: number | null;
  comments: ReviewComment[];
}

export interface PRInfo {
  number: number;
  title: string;
  body: string;
  headRef: string;
  state: string;
  author: string;
  threads: ReviewThread[];
}

/**
 * A review thread whose comments contained the trigger tag. Carries the
 * full context needed to drive a Claude session and post a reply when done.
 *
 * `instruction` is the trigger comment with the tag stripped — typically
 * something like "can we resolve this". The actionable feedback usually
 * lives in `originalCommentBody` (the root comment of the thread).
 */
export interface TriggeredThread {
  threadId: string;
  prNumber: number;
  prBranch: string;
  path: string;
  line: number | null;
  /** Trigger comment body with the trigger tag stripped. */
  instruction: string;
  /** Author of the trigger comment. */
  author: string;
  /**
   * Root comment of the thread — the original review feedback. May equal
   * `instruction` if the reviewer triggered the bot in their first comment.
   */
  originalCommentBody: string;
  /** Author of the root comment. */
  originalAuthor: string;
  /**
   * All comments in the thread joined into a single chronological string,
   * formatted as `@author: body\n---\n`. Useful for prompts that want the
   * whole conversation.
   */
  threadTranscript: string;
  diffHunk: string;
  commentNodeId: string;
  authorAssociation: string;
}

/** All triggered threads on a single PR scoped to one connector. */
export interface SubTask {
  connector: string;
  prNumber: number;
  prBranch: string;
  threads: TriggeredThread[];
}

export type SubTaskStatus =
  | "pending"
  | "resolving"
  | "building"
  | "build_failed"
  | "completed"
  | "failed"
  | "skipped";

/** Outcome of a single sub-task (one connector). */
export interface ResolveResult {
  connector: string;
  prNumber: number;
  fixedThreadIds: string[];
  failedThreadIds: string[];
  modifiedFiles: string[];
  buildPassed: boolean;
  clippyPassed: boolean;
  summary: string;
  error?: string;
  loopCount: number;
  /** Claude session id captured from the first call — used to resume in the fix loop. */
  claudeSessionId?: string;
}

/** Aggregated result of one poll cycle for the dashboard summary card. */
export interface CycleSummary {
  cycle: number;
  total: number;
  fixed: number;
  failed: number;
  skipped: number;
  queued: number;
  startedAt: number;
  completedAt: number;
}

/**
 * Wire shape for `pr-resolver:*` WebSocket events broadcast to the dashboard.
 * Kept loose so individual event handlers can attach extra fields without
 * forcing a global type bump.
 */
export interface PrResolverWsEvent {
  type: `pr-resolver:${string}`;
  payload: Record<string, unknown>;
}
