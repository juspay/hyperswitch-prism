import { StateManager } from "@byne/core";
import type { Leaf } from "./types.js";

interface DbLike {
  exec(sql: string): unknown;
  prepare(sql: string): {
    run(...args: unknown[]): unknown;
    get(...args: unknown[]): unknown;
    all(...args: unknown[]): unknown[];
  };
}

const PARITY_SCHEMA = `
CREATE TABLE IF NOT EXISTS parity_leaves (
  issue_number     INTEGER PRIMARY KEY,
  parent_tracking  INTEGER,
  connector        TEXT,
  flow             TEXT,
  title            TEXT,
  body             TEXT,
  labels_json      TEXT,
  linked_prs_json  TEXT,
  status           TEXT,
  locus            TEXT,
  pr_repo          TEXT,
  pr_number        INTEGER,
  understand_sid   TEXT,
  plan_sid         TEXT,
  execute_sid      TEXT,
  created_at       INTEGER,
  updated_at       INTEGER,
  last_seen_at     INTEGER
);

CREATE TABLE IF NOT EXISTS parity_heartbeats (
  heartbeat_id    TEXT PRIMARY KEY,
  started_at      INTEGER,
  completed_at    INTEGER,
  picked_leaf     INTEGER,
  outcome         TEXT,
  outcome_detail  TEXT
);
`;

function dbOf(state: StateManager): DbLike {
  const anyState = state as unknown as { db?: DbLike };
  if (!anyState.db) throw new Error("StateManager has no db handle (incompatible version)");
  return anyState.db;
}

export function extendForParity(state: StateManager): void {
  const db = dbOf(state);
  db.exec(PARITY_SCHEMA);
}

export interface LeafRow {
  issue_number: number;
  parent_tracking: number;
  connector: string;
  flow: string;
  title: string;
  body: string;
  labels_json: string;
  linked_prs_json: string;
  status: string;
  locus: string | null;
  pr_repo: string | null;
  pr_number: number | null;
  understand_sid: string | null;
  plan_sid: string | null;
  execute_sid: string | null;
  created_at: number;
  updated_at: number;
  last_seen_at: number;
}

export function upsertLeaf(state: StateManager, leaf: Leaf, status: string): void {
  const db = dbOf(state);
  const now = Date.now();
  const existing = db.prepare("SELECT issue_number FROM parity_leaves WHERE issue_number = ?").get(leaf.number);
  if (existing) {
    db.prepare(
      `UPDATE parity_leaves
       SET parent_tracking=@parent, connector=@conn, flow=@flow, title=@title, body=@body,
           labels_json=@labels, linked_prs_json=@prs, status=@status,
           updated_at=@now, last_seen_at=@now
       WHERE issue_number=@n`,
    ).run({
      parent: leaf.parentTracking,
      conn: leaf.connector,
      flow: leaf.flow,
      title: leaf.title,
      body: leaf.body,
      labels: JSON.stringify(leaf.labels),
      prs: JSON.stringify(leaf.linkedPRs),
      status,
      now,
      n: leaf.number,
    });
  } else {
    db.prepare(
      `INSERT INTO parity_leaves (issue_number, parent_tracking, connector, flow, title, body,
         labels_json, linked_prs_json, status, locus, pr_repo, pr_number,
         understand_sid, plan_sid, execute_sid,
         created_at, updated_at, last_seen_at)
       VALUES (@n,@parent,@conn,@flow,@title,@body,@labels,@prs,@status,NULL,NULL,NULL,NULL,NULL,NULL,@now,@now,@now)`,
    ).run({
      n: leaf.number,
      parent: leaf.parentTracking,
      conn: leaf.connector,
      flow: leaf.flow,
      title: leaf.title,
      body: leaf.body,
      labels: JSON.stringify(leaf.labels),
      prs: JSON.stringify(leaf.linkedPRs),
      status,
      now,
    });
  }
}

export function getLeafRow(state: StateManager, num: number): LeafRow | null {
  const db = dbOf(state);
  const r = db.prepare("SELECT * FROM parity_leaves WHERE issue_number = ?").get(num) as LeafRow | undefined;
  return r ?? null;
}

export function saveSessionIds(state: StateManager, num: number, ids: { understand?: string; plan?: string; execute?: string }): void {
  const db = dbOf(state);
  db.prepare(
    `UPDATE parity_leaves
     SET understand_sid = COALESCE(@u, understand_sid),
         plan_sid       = COALESCE(@p, plan_sid),
         execute_sid    = COALESCE(@e, execute_sid),
         updated_at     = @now
     WHERE issue_number = @n`,
  ).run({ u: ids.understand ?? null, p: ids.plan ?? null, e: ids.execute ?? null, now: Date.now(), n: num });
}

export function recordHeartbeat(state: StateManager, opts: { id: string; startedAt: number; completedAt?: number; pickedLeaf?: number; outcome: string; detail?: string }): void {
  const db = dbOf(state);
  db.prepare(
    `INSERT INTO parity_heartbeats (heartbeat_id, started_at, completed_at, picked_leaf, outcome, outcome_detail)
     VALUES (@id, @start, @end, @pick, @out, @detail)
     ON CONFLICT(heartbeat_id) DO UPDATE SET completed_at=@end, picked_leaf=@pick, outcome=@out, outcome_detail=@detail`,
  ).run({ id: opts.id, start: opts.startedAt, end: opts.completedAt ?? null, pick: opts.pickedLeaf ?? null, out: opts.outcome, detail: opts.detail ?? null });
}

export function newHeartbeatId(): string {
  return `hb-${new Date().toISOString().replace(/[^0-9]/g, "")}-${process.pid}`;
}
