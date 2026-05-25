import Database from "better-sqlite3";
import path from "node:path";
import fs from "node:fs";
import type {
  AttemptRecord,
  CheckpointId,
  CheckpointStatus,
  PipelineContext,
  TaskDefinition,
} from "./types.js";

export interface SavedState {
  runId: string;
  task: TaskDefinition;
  artifacts: Record<string, unknown>;
  retryCount: Record<string, number>;
  checkpointStates: Record<string, CheckpointStatus>;
  /** Per-attempt history rehydrated from the checkpoint_attempts table. */
  attempts?: AttemptRecord[];
}

export interface RunSummary {
  runId: string;
  title: string;
  sessionId: string;
  status: RunStatus;
  createdAt: number;
  updatedAt: number;
  lastCheckpoint?: CheckpointId;
  lastStatus?: CheckpointStatus;
}

export interface CheckpointHistory {
  checkpointId: CheckpointId;
  status: CheckpointStatus;
  updatedAt: number;
}

export type RunStatus =
  | "pending"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled";

export type SessionStatus = "idle" | "running" | "cancelling" | "error" | "archived";

export type SessionCopyStrategy = "git-worktree" | "full" | "shallow" | "legacy";

export interface SessionMetadata {
  originalPath: string;
  copyStrategy: SessionCopyStrategy;
  diskUsageBytes?: number;
  /**
   * Initial task to auto-submit when the session starts.
   * Set when creating a session with the unified modal.
   */
  initialTask?: TaskDefinition;
}

export interface SessionRecord {
  sessionId: string;
  name: string;
  description: string | null;
  projectRoot: string;
  currentRunId: string | null;
  status: SessionStatus;
  wsPort: number | null;
  pid: number | null;
  /**
   * Phase 10: per-session offset for gRPC/dummy-connector ports so parallel
   * sessions don't fight for the same listeners. Session N uses 8000+N for
   * gRPC and 8080+N for dummy-connector. Default session keeps slot 0 so it
   * uses the original unshifted ports (8000/8080) for back-compat. Assigned
   * at SessionManager.create() via StateManager.allocateNextPortSlot().
   */
  portSlot: number;
  createdAt: number;
  updatedAt: number;
  metadata: SessionMetadata;
}

export interface CheckpointEvent {
  runId: string;
  checkpointId: CheckpointId;
  eventType: "started" | "passed" | "failed" | "retry";
  attemptNumber?: number;
  timestamp?: number;
  durationMs?: number;
  errorMessage?: string;
  metadata?: Record<string, unknown>;
}

export interface SessionActivityEvent {
  sessionId: string;
  activityType:
    | "created"
    | "run_started"
    | "run_completed"
    | "archived"
    | "deleted";
  runId?: string;
  timestamp?: number;
  metadata?: Record<string, unknown>;
}

export const DEFAULT_SESSION_ID = "default";

/**
 * Schema for the *initial* (pre-session) database. Kept verbatim so first-time
 * installs hit `IF NOT EXISTS` paths and existing databases get upgraded by
 * `runMigrations()`.
 */
const BASE_SCHEMA = `
CREATE TABLE IF NOT EXISTS runs (
  run_id TEXT PRIMARY KEY,
  task_json TEXT NOT NULL,
  artifacts_json TEXT NOT NULL,
  retry_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS checkpoint_states (
  run_id TEXT NOT NULL,
  checkpoint_id TEXT NOT NULL,
  status TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (run_id, checkpoint_id)
);
CREATE TABLE IF NOT EXISTS checkpoint_attempts (
  run_id TEXT NOT NULL,
  checkpoint_id TEXT NOT NULL,
  attempt_index INTEGER NOT NULL,
  status TEXT NOT NULL,
  errors_json TEXT,
  output TEXT,
  artifacts_json TEXT,
  started_at INTEGER,
  completed_at INTEGER NOT NULL,
  PRIMARY KEY (run_id, checkpoint_id, attempt_index)
);
CREATE INDEX IF NOT EXISTS idx_attempts_run_cp
  ON checkpoint_attempts(run_id, checkpoint_id);
`;

/**
 * Session-management schema. Applied on top of BASE_SCHEMA via PRAGMA-gated
 * migrations so existing pre-v1 databases pick up the new tables and columns.
 *
 * Indexes that touch newly-added columns on `runs` live in
 * `RUNS_SESSION_INDEXES` so they're created *after* the ALTER TABLE step.
 */
const SESSION_SCHEMA = `
CREATE TABLE IF NOT EXISTS sessions (
  session_id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  project_root TEXT NOT NULL,
  current_run_id TEXT,
  status TEXT NOT NULL DEFAULT 'idle',
  ws_port INTEGER,
  pid INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  metadata_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS checkpoint_events (
  event_id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id TEXT NOT NULL,
  checkpoint_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  attempt_number INTEGER DEFAULT 0,
  timestamp INTEGER NOT NULL,
  duration_ms INTEGER,
  error_message TEXT,
  metadata_json TEXT
);
CREATE TABLE IF NOT EXISTS session_activity (
  activity_id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  activity_type TEXT NOT NULL,
  run_id TEXT,
  timestamp INTEGER NOT NULL,
  metadata_json TEXT
);
CREATE INDEX IF NOT EXISTS idx_sessions_current_run ON sessions(current_run_id);
CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
CREATE INDEX IF NOT EXISTS idx_checkpoint_events_run ON checkpoint_events(run_id);
CREATE INDEX IF NOT EXISTS idx_checkpoint_events_type ON checkpoint_events(event_type, timestamp);
CREATE INDEX IF NOT EXISTS idx_session_activity_session ON session_activity(session_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_session_activity_type ON session_activity(activity_type, timestamp);
`;

/** Indexes that depend on columns added by ALTER TABLE in v0→v1 migration. */
const RUNS_SESSION_INDEXES = `
CREATE INDEX IF NOT EXISTS idx_runs_session ON runs(session_id);
CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
`;

export class StateManager {
  private db: Database.Database;

  constructor(dbPath?: string) {
    const p =
      dbPath ??
      path.join(process.env.HOME ?? ".", ".10xgrace", "pipeline.sqlite");
    fs.mkdirSync(path.dirname(p), { recursive: true });
    this.db = new Database(p);
    this.db.pragma("journal_mode = WAL");
    this.db.exec(BASE_SCHEMA);
    this.runMigrations();
  }

  /**
   * Idempotent schema migrations gated by `PRAGMA user_version`.
   * - v0 → v1: session-management tables, ALTER runs to add session_id +
   *   lifecycle columns, backfill a default session row, mark legacy runs
   *   as `succeeded`.
   * - v1 → v2 (Phase 10): ALTER sessions to add `port_slot` column,
   *   backfill default session to slot 0 and existing extra sessions to
   *   sequential slots in created_at order so they stop fighting for the
   *   same gRPC/dummy ports.
   *
   * Wrapped in BEGIN IMMEDIATE so concurrent boots can't half-apply.
   */
  private runMigrations(): void {
    const current = (this.db.pragma("user_version", { simple: true }) as number) ?? 0;
    if (current >= 2) return;

    const tx = this.db.transaction(() => {
      if (current < 1) {
        // ─── v0 → v1 ────────────────────────────────────────────────────
        // 1. Sessions tables + indexes (idempotent on re-run).
        this.db.exec(SESSION_SCHEMA);

        // 2. Add columns to runs only if they don't already exist.
        const cols = (this.db.prepare(`PRAGMA table_info(runs)`).all() as {
          name: string;
        }[]).map((c) => c.name);
        const addColumn = (name: string, def: string) => {
          if (!cols.includes(name)) {
            this.db.exec(`ALTER TABLE runs ADD COLUMN ${name} ${def}`);
          }
        };
        addColumn("session_id", "TEXT");
        addColumn("status", "TEXT DEFAULT 'pending'");
        addColumn("heartbeat_at", "INTEGER");
        addColumn("started_at", "INTEGER");
        addColumn("completed_at", "INTEGER");
        addColumn("duration_ms", "INTEGER");

        // 2b. Now that the new columns exist, create their indexes.
        this.db.exec(RUNS_SESSION_INDEXES);

        // 3. Backfill: create default session pointing at the existing
        //    config.projectRoot. We don't have access to the config from
        //    inside StateManager, so we plant a placeholder; the caller
        //    (run.ts) overwrites it on boot once config is loaded.
        const existing = this.db
          .prepare(`SELECT 1 FROM sessions WHERE session_id = ?`)
          .get(DEFAULT_SESSION_ID);
        if (!existing) {
          const now = Date.now();
          this.db
            .prepare(
              `INSERT INTO sessions (
                session_id, name, description, project_root,
                current_run_id, status, ws_port, pid,
                created_at, updated_at, metadata_json
              ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
            )
            .run(
              DEFAULT_SESSION_ID,
              "Default Session",
              "Auto-created during session-management migration. Pre-existing runs are linked here.",
              "", // populated by ensureDefaultSession() at boot
              null,
              "idle",
              null,
              null,
              now,
              now,
              JSON.stringify({
                originalPath: "",
                copyStrategy: "legacy",
              } satisfies SessionMetadata)
            );
        }

        // 4. Backfill: link orphan runs to default session, mark them succeeded
        //    (we can't tell from on-disk state whether they finished cleanly,
        //    but treating prior runs as historical avoids spurious "running"
        //    states blocking the default session lock).
        this.db.exec(`
          UPDATE runs
             SET session_id = '${DEFAULT_SESSION_ID}'
           WHERE session_id IS NULL OR session_id = '';
        `);
        this.db.exec(`
          UPDATE runs
             SET status = 'succeeded'
           WHERE status IS NULL OR status = '' OR status = 'pending';
        `);

        this.db.pragma("user_version = 1");
      }

      // ─── v1 → v2 ──────────────────────────────────────────────────────
      // Phase 10: add per-session port_slot column so parallel sessions
      // get distinct gRPC + dummy-connector ports.
      const sessCols = (this.db.prepare(`PRAGMA table_info(sessions)`).all() as {
        name: string;
      }[]).map((c) => c.name);
      if (!sessCols.includes("port_slot")) {
        this.db.exec(`ALTER TABLE sessions ADD COLUMN port_slot INTEGER`);
      }
      // Default session always gets slot 0 (so it keeps the original
      // unshifted 8000/8080 ports for backward compatibility).
      this.db
        .prepare(
          `UPDATE sessions SET port_slot = 0 WHERE session_id = ? AND (port_slot IS NULL OR port_slot != 0)`
        )
        .run(DEFAULT_SESSION_ID);
      // Backfill every other existing session with sequential slots in
      // created_at order. ROW_NUMBER() is available since SQLite 3.25
      // which better-sqlite3 ships with.
      this.db.exec(`
        WITH ordered AS (
          SELECT session_id, ROW_NUMBER() OVER (ORDER BY created_at) AS slot
            FROM sessions
           WHERE session_id != '${DEFAULT_SESSION_ID}'
             AND port_slot IS NULL
        )
        UPDATE sessions
           SET port_slot = (SELECT slot FROM ordered WHERE ordered.session_id = sessions.session_id)
         WHERE session_id IN (SELECT session_id FROM ordered);
      `);

      this.db.pragma("user_version = 2");
    });
    tx.immediate();
  }

  /**
   * Make sure a default session exists with the supplied projectRoot. Called
   * by run.ts on boot so the placeholder planted during migration gets a real
   * path to point at.
   */
  ensureDefaultSession(projectRoot: string): SessionRecord {
    const now = Date.now();
    const existing = this.getSession(DEFAULT_SESSION_ID);
    if (existing) {
      if (!existing.projectRoot) {
        this.db
          .prepare(
            `UPDATE sessions
                SET project_root = @projectRoot,
                    metadata_json = @metadata,
                    updated_at = @now
              WHERE session_id = @id`
          )
          .run({
            id: DEFAULT_SESSION_ID,
            projectRoot,
            metadata: JSON.stringify({
              ...existing.metadata,
              originalPath: projectRoot,
            } satisfies SessionMetadata),
            now,
          });
        return this.getSession(DEFAULT_SESSION_ID)!;
      }
      return existing;
    }
    this.db
      .prepare(
        `INSERT INTO sessions (
          session_id, name, description, project_root,
          current_run_id, status, ws_port, pid, port_slot,
          created_at, updated_at, metadata_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
      )
      .run(
        DEFAULT_SESSION_ID,
        "Default Session",
        null,
        projectRoot,
        null,
        "idle",
        null,
        null,
        0, // Phase 10: default session keeps slot 0 (unshifted 8000/8080)
        now,
        now,
        JSON.stringify({
          originalPath: projectRoot,
          copyStrategy: "legacy",
        } satisfies SessionMetadata)
      );
    return this.getSession(DEFAULT_SESSION_ID)!;
  }

  // ─── Session CRUD ────────────────────────────────────────────────────────

  createSession(input: {
    sessionId: string;
    name: string;
    description?: string | null;
    projectRoot: string;
    metadata: SessionMetadata;
    /** Phase 10: allocated by SessionManager via allocateNextPortSlot. */
    portSlot: number;
  }): SessionRecord {
    const now = Date.now();
    this.db
      .prepare(
        `INSERT INTO sessions (
          session_id, name, description, project_root,
          current_run_id, status, ws_port, pid, port_slot,
          created_at, updated_at, metadata_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
      )
      .run(
        input.sessionId,
        input.name,
        input.description ?? null,
        input.projectRoot,
        null,
        "idle",
        null,
        null,
        input.portSlot,
        now,
        now,
        JSON.stringify(input.metadata)
      );
    this.recordSessionActivity({
      sessionId: input.sessionId,
      activityType: "created",
    });
    return this.getSession(input.sessionId)!;
  }

  getSession(sessionId: string): SessionRecord | null {
    const row = this.db
      .prepare(`SELECT * FROM sessions WHERE session_id = ?`)
      .get(sessionId) as
      | {
          session_id: string;
          name: string;
          description: string | null;
          project_root: string;
          current_run_id: string | null;
          status: SessionStatus;
          ws_port: number | null;
          pid: number | null;
          port_slot: number | null;
          created_at: number;
          updated_at: number;
          metadata_json: string;
        }
      | undefined;
    if (!row) return null;
    return rowToSession(row);
  }

  /**
   * Phase 10: allocate the smallest non-negative integer not currently in
   * use as a port_slot. Called by SessionManager.create() right before
   * insert. Bounded at 1000 to keep the search cheap — realistic worst
   * case is a few dozen concurrent sessions.
   */
  allocateNextPortSlot(): number {
    const rows = this.db
      .prepare(`SELECT port_slot FROM sessions WHERE port_slot IS NOT NULL`)
      .all() as { port_slot: number }[];
    const used = new Set(rows.map((r) => r.port_slot));
    for (let i = 0; i < 1000; i++) {
      if (!used.has(i)) return i;
    }
    throw new Error(
      "Port slot pool exhausted (>1000 sessions). Delete old sessions before creating more."
    );
  }

  listSessions(): SessionRecord[] {
    const rows = this.db
      .prepare(`SELECT * FROM sessions ORDER BY updated_at DESC`)
      .all() as Parameters<typeof rowToSession>[0][];
    return rows.map(rowToSession);
  }

  archiveSession(sessionId: string): void {
    this.db
      .prepare(
        `UPDATE sessions
            SET status = 'archived', updated_at = ?
          WHERE session_id = ?`
      )
      .run(Date.now(), sessionId);
    this.recordSessionActivity({ sessionId, activityType: "archived" });
  }

  /**
   * Drop a session row. Caller is responsible for removing the on-disk
   * worktree (SessionManager.delete handles that).
   */
  deleteSession(sessionId: string): void {
    if (sessionId === DEFAULT_SESSION_ID) {
      throw new Error("Cannot delete the default session");
    }
    this.recordSessionActivity({ sessionId, activityType: "deleted" });
    this.db.prepare(`DELETE FROM sessions WHERE session_id = ?`).run(sessionId);
  }

  updateSessionRuntime(
    sessionId: string,
    update: { wsPort?: number | null; pid?: number | null }
  ): void {
    this.db
      .prepare(
        `UPDATE sessions
             SET ws_port = COALESCE(@wsPort, ws_port),
                 pid     = COALESCE(@pid, pid),
                 updated_at = @now
           WHERE session_id = @sessionId`
      )
      .run({
        sessionId,
        wsPort: update.wsPort ?? null,
        pid: update.pid ?? null,
        now: Date.now(),
      });
  }

  updateSessionMetadata(sessionId: string, metadata: SessionMetadata): void {
    this.db
      .prepare(
        `UPDATE sessions
             SET metadata_json = @metadata,
                 updated_at = @now
           WHERE session_id = @sessionId`
      )
      .run({
        sessionId,
        metadata: JSON.stringify(metadata),
        now: Date.now(),
      });
  }

  // ─── Concurrency primitives ──────────────────────────────────────────────

  /**
   * Atomically claim a session for a run. Returns true if the lock was
   * acquired; false if another run already holds it. Re-claiming the same
   * (sessionId, runId) pair is a no-op success — useful for engine resume
   * paths that re-enter run().
   */
  claimSession(sessionId: string, runId: string): boolean {
    const stmt = this.db.prepare(`
      UPDATE sessions
         SET current_run_id = @runId,
             status = 'running',
             updated_at = @now
       WHERE session_id = @sessionId
         AND (current_run_id IS NULL OR current_run_id = @runId)
    `);
    const result = stmt.run({ sessionId, runId, now: Date.now() });
    if (result.changes > 0) {
      this.recordSessionActivity({
        sessionId,
        activityType: "run_started",
        runId,
      });
    }
    return result.changes > 0;
  }

  /**
   * Release a session's lock and stamp the run's terminal status. Idempotent —
   * calling this twice with the same finalStatus is harmless.
   */
  releaseSession(
    sessionId: string,
    runId: string,
    finalStatus: "succeeded" | "failed" | "cancelled"
  ): void {
    const tx = this.db.transaction(() => {
      const now = Date.now();
      this.db
        .prepare(
          `UPDATE sessions
              SET current_run_id = NULL,
                  status = 'idle',
                  updated_at = @now
            WHERE session_id = @sessionId
              AND current_run_id = @runId`
        )
        .run({ sessionId, runId, now });

      // Compute duration from started_at when present.
      const row = this.db
        .prepare(`SELECT started_at FROM runs WHERE run_id = ?`)
        .get(runId) as { started_at: number | null } | undefined;
      const duration = row?.started_at != null ? now - row.started_at : null;

      this.db
        .prepare(
          `UPDATE runs
              SET status = @status,
                  heartbeat_at = NULL,
                  completed_at = @now,
                  duration_ms = @duration,
                  updated_at = @now
            WHERE run_id = @runId`
        )
        .run({ runId, status: finalStatus, now, duration });

      this.recordSessionActivity({
        sessionId,
        activityType: "run_completed",
        runId,
        metadata: { status: finalStatus, durationMs: duration ?? undefined },
      });
    });
    tx();
  }

  /**
   * Liveness ping. Called between checkpoints. No-op if the run isn't
   * currently in `running` status, so this is safe to call from anywhere.
   */
  heartbeat(runId: string): void {
    const now = Date.now();
    this.db
      .prepare(
        `UPDATE runs
            SET heartbeat_at = @now,
                updated_at = @now
          WHERE run_id = @runId AND status = 'running'`
      )
      .run({ runId, now });
  }

  /**
   * Mark a run as actively running (status='running', stamp started_at on
   * first transition). Called by the engine right before the checkpoint
   * loop begins.
   */
  markRunRunning(runId: string): void {
    const now = Date.now();
    this.db
      .prepare(
        `UPDATE runs
            SET status = 'running',
                started_at = COALESCE(started_at, @now),
                heartbeat_at = @now,
                updated_at = @now
          WHERE run_id = @runId`
      )
      .run({ runId, now });
  }

  /**
   * Reap stale sessions whose runs haven't beat in `timeoutMs`. Used on
   * engine boot to clean up after crashes.
   */
  recoverStaleSessions(timeoutMs = 60_000): number {
    const cutoff = Date.now() - timeoutMs;
    const now = Date.now();
    let cleared = 0;
    const tx = this.db.transaction(() => {
      const stale = this.db
        .prepare(
          `SELECT run_id FROM runs
             WHERE status = 'running'
               AND (heartbeat_at IS NULL OR heartbeat_at < ?)`
        )
        .all(cutoff) as { run_id: string }[];
      cleared = stale.length;
      if (cleared === 0) return;

      this.db
        .prepare(
          `UPDATE sessions
              SET current_run_id = NULL,
                  status = 'error',
                  updated_at = ?
            WHERE current_run_id IN (
              SELECT run_id FROM runs
                WHERE status = 'running'
                  AND (heartbeat_at IS NULL OR heartbeat_at < ?)
            )`
        )
        .run(now, cutoff);

      this.db
        .prepare(
          `UPDATE runs
              SET status = 'failed',
                  heartbeat_at = NULL,
                  completed_at = ?,
                  updated_at = ?
            WHERE status = 'running'
              AND (heartbeat_at IS NULL OR heartbeat_at < ?)`
        )
        .run(now, now, cutoff);
    });
    tx();
    return cleared;
  }

  /**
   * Insert a pending run row tied to a session. Returns the new runId.
   * The supervisor / worker loop is responsible for picking it up.
   */
  enqueueRun(sessionId: string, runId: string, task: TaskDefinition): void {
    const now = Date.now();
    // Include task in artifacts so it's available via ctx.artifacts.task in checkpoints
    const artifacts = { task };
    this.db
      .prepare(
        `INSERT INTO runs (
          run_id, session_id, task_json, artifacts_json, retry_json,
          status, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`
      )
      .run(
        runId,
        sessionId,
        JSON.stringify(task),
        JSON.stringify(artifacts),
        "{}",
        "pending",
        now,
        now
      );
  }

  // ─── Analytics events ────────────────────────────────────────────────────

  recordCheckpointEvent(event: CheckpointEvent): void {
    this.db
      .prepare(
        `INSERT INTO checkpoint_events (
          run_id, checkpoint_id, event_type, attempt_number, timestamp,
          duration_ms, error_message, metadata_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`
      )
      .run(
        event.runId,
        event.checkpointId,
        event.eventType,
        event.attemptNumber ?? 0,
        event.timestamp ?? Date.now(),
        event.durationMs ?? null,
        event.errorMessage ?? null,
        event.metadata ? JSON.stringify(event.metadata) : null
      );
  }

  recordSessionActivity(event: SessionActivityEvent): void {
    this.db
      .prepare(
        `INSERT INTO session_activity (
          session_id, activity_type, run_id, timestamp, metadata_json
        ) VALUES (?, ?, ?, ?, ?)`
      )
      .run(
        event.sessionId,
        event.activityType,
        event.runId ?? null,
        event.timestamp ?? Date.now(),
        event.metadata ? JSON.stringify(event.metadata) : null
      );
  }

  async save(
    ctx: PipelineContext,
    checkpointId: CheckpointId,
    status: CheckpointStatus
  ): Promise<void> {
    const now = Date.now();
    const sessionId = ctx.sessionId ?? DEFAULT_SESSION_ID;
    const upsertRun = this.db.prepare(`
      INSERT INTO runs (
        run_id, session_id, task_json, artifacts_json, retry_json,
        status, created_at, updated_at
      )
      VALUES (
        @run_id, @session_id, @task_json, @artifacts_json, @retry_json,
        'running', @created_at, @updated_at
      )
      ON CONFLICT(run_id) DO UPDATE SET
        task_json = excluded.task_json,
        artifacts_json = excluded.artifacts_json,
        retry_json = excluded.retry_json,
        updated_at = excluded.updated_at
    `);
    upsertRun.run({
      run_id: ctx.runId,
      session_id: sessionId,
      task_json: JSON.stringify(ctx.task),
      artifacts_json: JSON.stringify(ctx.artifacts),
      retry_json: JSON.stringify(ctx.retryCount),
      created_at: now,
      updated_at: now,
    });

    const upsertCp = this.db.prepare(`
      INSERT INTO checkpoint_states (run_id, checkpoint_id, status, updated_at)
      VALUES (?, ?, ?, ?)
      ON CONFLICT(run_id, checkpoint_id) DO UPDATE SET
        status = excluded.status,
        updated_at = excluded.updated_at
    `);
    upsertCp.run(ctx.runId, checkpointId, status, now);
  }

  /**
   * Record a single checkpoint attempt. Called by the engine after every
   * pass-or-fail completion, before any state mutation that would lose the
   * data (the `Object.assign(ctx.artifacts, …)` merge on the next attempt).
   *
   * INSERT OR REPLACE on the (run_id, checkpoint_id, attempt_index) primary
   * key, so re-emitting the same attempt is idempotent.
   */
  async saveAttempt(
    runId: string,
    checkpointId: CheckpointId,
    attemptIndex: number,
    status: "passed" | "failed",
    errors: string[] | null,
    output: string | null,
    artifacts: Record<string, unknown> | null,
    startedAt: number | null,
    completedAt: number
  ): Promise<void> {
    const stmt = this.db.prepare(`
      INSERT INTO checkpoint_attempts (
        run_id, checkpoint_id, attempt_index, status,
        errors_json, output, artifacts_json, started_at, completed_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(run_id, checkpoint_id, attempt_index) DO UPDATE SET
        status        = excluded.status,
        errors_json   = excluded.errors_json,
        output        = excluded.output,
        artifacts_json = excluded.artifacts_json,
        started_at    = excluded.started_at,
        completed_at  = excluded.completed_at
    `);
    stmt.run(
      runId,
      checkpointId,
      attemptIndex,
      status,
      errors && errors.length > 0 ? JSON.stringify(errors) : null,
      output,
      artifacts ? JSON.stringify(artifacts) : null,
      startedAt,
      completedAt
    );
  }

  /**
   * Return all attempts for a run, ordered by completion time. The dashboard
   * calls this on connect via the `attempts:request` WS message so retry
   * history survives browser reloads.
   */
  async listAttempts(runId: string): Promise<AttemptRecord[]> {
    const rows = this.db
      .prepare(
        `SELECT checkpoint_id, attempt_index, status, errors_json, output,
                artifacts_json, started_at, completed_at
           FROM checkpoint_attempts
          WHERE run_id = ?
          ORDER BY completed_at ASC`
      )
      .all(runId) as {
      checkpoint_id: CheckpointId;
      attempt_index: number;
      status: "passed" | "failed";
      errors_json: string | null;
      output: string | null;
      artifacts_json: string | null;
      started_at: number | null;
      completed_at: number;
    }[];
    return rows.map((r) => ({
      runId,
      checkpointId: r.checkpoint_id,
      attemptIndex: r.attempt_index,
      status: r.status,
      errors: r.errors_json ? (JSON.parse(r.errors_json) as string[]) : undefined,
      output: r.output,
      artifacts: r.artifacts_json
        ? (JSON.parse(r.artifacts_json) as Record<string, unknown>)
        : null,
      startedAt: r.started_at ?? undefined,
      completedAt: r.completed_at,
    }));
  }

  async load(runId: string): Promise<SavedState | null> {
    const row = this.db
      .prepare(`SELECT * FROM runs WHERE run_id = ?`)
      .get(runId) as
      | {
          run_id: string;
          task_json: string;
          artifacts_json: string;
          retry_json: string;
        }
      | undefined;
    if (!row) return null;

    const cpRows = this.db
      .prepare(
        `SELECT checkpoint_id, status FROM checkpoint_states WHERE run_id = ?`
      )
      .all(runId) as { checkpoint_id: string; status: CheckpointStatus }[];
    const checkpointStates: Record<string, CheckpointStatus> = {};
    for (const r of cpRows) checkpointStates[r.checkpoint_id] = r.status;

    const attempts = await this.listAttempts(runId);

    return {
      runId: row.run_id,
      task: JSON.parse(row.task_json),
      artifacts: JSON.parse(row.artifacts_json),
      retryCount: JSON.parse(row.retry_json),
      checkpointStates,
      attempts,
    };
  }

  async listRuns(sessionId?: string): Promise<RunSummary[]> {
    const where = sessionId ? `WHERE session_id = ?` : ``;
    const params = sessionId ? [sessionId] : [];
    const rows = this.db
      .prepare(
        `SELECT run_id, session_id, status, task_json, created_at, updated_at
           FROM runs ${where}
          ORDER BY updated_at DESC`
      )
      .all(...params) as {
      run_id: string;
      session_id: string | null;
      status: RunStatus | null;
      task_json: string;
      created_at: number;
      updated_at: number;
    }[];
    return rows.map((r) => {
      const task = JSON.parse(r.task_json) as TaskDefinition;
      const last = this.db
        .prepare(
          `SELECT checkpoint_id, status FROM checkpoint_states WHERE run_id = ? ORDER BY updated_at DESC LIMIT 1`
        )
        .get(r.run_id) as
        | { checkpoint_id: CheckpointId; status: CheckpointStatus }
        | undefined;
      return {
        runId: r.run_id,
        title: task.title ?? "(untitled)",
        sessionId: r.session_id ?? DEFAULT_SESSION_ID,
        status: (r.status ?? "succeeded") as RunStatus,
        createdAt: r.created_at,
        updatedAt: r.updated_at,
        lastCheckpoint: last?.checkpoint_id,
        lastStatus: last?.status,
      };
    });
  }

  /**
   * Rewind a run: delete saved checkpoint_states rows for the given stages
   * (so the dashboard shows them as idle on reconnect) and overwrite the
   * stored artifacts/retry-counts with the cleaned versions from memory.
   * Used by the resume flow when the user re-runs from a specific step.
   */
  async rewindRun(
    runId: string,
    artifacts: Record<string, unknown>,
    retryCount: Record<string, number>,
    clearedStages: CheckpointId[]
  ): Promise<void> {
    const delStmt = this.db.prepare(
      `DELETE FROM checkpoint_states WHERE run_id = ? AND checkpoint_id = ?`
    );
    const tx = this.db.transaction(() => {
      for (const id of clearedStages) delStmt.run(runId, id);
      this.db
        .prepare(
          `UPDATE runs
             SET artifacts_json = @artifacts_json,
                 retry_json     = @retry_json,
                 updated_at     = @updated_at
           WHERE run_id = @run_id`
        )
        .run({
          run_id: runId,
          artifacts_json: JSON.stringify(artifacts),
          retry_json: JSON.stringify(retryCount),
          updated_at: Date.now(),
        });
    });
    tx();
  }

  async getCheckpointHistory(runId: string): Promise<CheckpointHistory[]> {
    const rows = this.db
      .prepare(
        `SELECT checkpoint_id, status, updated_at FROM checkpoint_states WHERE run_id = ? ORDER BY updated_at ASC`
      )
      .all(runId) as {
      checkpoint_id: CheckpointId;
      status: CheckpointStatus;
      updated_at: number;
    }[];
    return rows.map((r) => ({
      checkpointId: r.checkpoint_id,
      status: r.status,
      updatedAt: r.updated_at,
    }));
  }

  /**
   * Remove runs that are *clearly* abandoned scaffolding:
   *  - empty / missing task title AND no passed checkpoint
   * A run with a real title OR at least one passed stage is preserved,
   * even if it was later abandoned — those represent real progress.
   */
  async pruneEmptyRuns(): Promise<number> {
    // Pending and running rows are intentionally untitled — the supervisor
    // seeds them via enqueueRun() before the child engine fills in the task
    // from UI input. Pruning those would race with the spawn handshake.
    const rows = this.db
      .prepare(
        `SELECT run_id, task_json FROM runs
          WHERE status IS NULL OR status NOT IN ('pending', 'running')`
      )
      .all() as { run_id: string; task_json: string }[];
    let removed = 0;
    const deleteRun = this.db.prepare(`DELETE FROM runs WHERE run_id = ?`);
    const deleteStates = this.db.prepare(
      `DELETE FROM checkpoint_states WHERE run_id = ?`
    );
    const deleteAttempts = this.db.prepare(
      `DELETE FROM checkpoint_attempts WHERE run_id = ?`
    );
    const anyPassed = this.db.prepare(
      `SELECT 1 FROM checkpoint_states WHERE run_id = ? AND status = 'passed' LIMIT 1`
    );
    for (const r of rows) {
      let title = "";
      try {
        const t = JSON.parse(r.task_json) as { title?: string };
        title = t.title?.trim() ?? "";
      } catch {
        continue;
      }
      const hasPassed = anyPassed.get(r.run_id);
      if (!title && !hasPassed) {
        deleteAttempts.run(r.run_id);
        deleteStates.run(r.run_id);
        deleteRun.run(r.run_id);
        removed++;
      }
    }
    return removed;
  }

  close() {
    this.db.close();
  }
}

function rowToSession(row: {
  session_id: string;
  name: string;
  description: string | null;
  project_root: string;
  current_run_id: string | null;
  status: SessionStatus;
  ws_port: number | null;
  pid: number | null;
  port_slot: number | null;
  created_at: number;
  updated_at: number;
  metadata_json: string;
}): SessionRecord {
  let metadata: SessionMetadata = {
    originalPath: row.project_root,
    copyStrategy: "legacy",
  };
  try {
    metadata = JSON.parse(row.metadata_json) as SessionMetadata;
  } catch {
    /* keep fallback */
  }
  return {
    sessionId: row.session_id,
    name: row.name,
    description: row.description,
    projectRoot: row.project_root,
    currentRunId: row.current_run_id,
    status: row.status,
    wsPort: row.ws_port,
    pid: row.pid,
    // Phase 10: null means "pre-v2 row that the migration hasn't touched yet";
    // fall back to slot 0 (unshifted ports) for safety.
    portSlot: row.port_slot ?? 0,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    metadata,
  };
}
