import { join } from "node:path";
import { writeAtomic } from "../cache.js";
import type { ParityConfig } from "../config.js";
import type { Leaf } from "../types.js";
import { deriveStatus } from "./derive.js";

function nowIso(): string {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}

interface ConnectorBucket {
  name: string;
  trackingNumbers: Set<number>;
  leaves: Leaf[];
}

function group(leaves: Leaf[]): ConnectorBucket[] {
  const byName = new Map<string, ConnectorBucket>();
  for (const l of leaves) {
    const k = l.connector || "unknown";
    let b = byName.get(k);
    if (!b) {
      b = { name: k, trackingNumbers: new Set(), leaves: [] };
      byName.set(k, b);
    }
    b.trackingNumbers.add(l.parentTracking);
    b.leaves.push(l);
  }
  return [...byName.values()].sort((a, b) => a.name.localeCompare(b.name));
}

function countByStatus(leaves: Leaf[]): Record<string, number> {
  const c = { "no-pr": 0, "pr-open": 0, "pr-merged": 0, blocked: 0, "not-applicable": 0, closed: 0 } as Record<string, number>;
  for (const l of leaves) c[deriveStatus(l)]++;
  return c;
}

function latestForConnector(leaves: Leaf[]): string {
  const sorted = [...leaves].sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1));
  const top = sorted[0];
  if (!top) return "—";
  return `#${top.number} ${deriveStatus(top)}`;
}

export function renderDashboardMarkdown(leaves: Leaf[], cfg: ParityConfig): string {
  const buckets = group(leaves);
  const lines: string[] = [];
  lines.push(`# Parity Dashboard — last refresh: ${nowIso()}`);
  lines.push(`Root: ${cfg.github.owner}/${cfg.github.repo}#${cfg.github.rootIssue}`);
  lines.push("");
  lines.push("| Connector | Tracking | Leaves | No PR | PR open | Merged | Blocked | Latest |");
  lines.push("|-----------|----------|--------|-------|---------|--------|---------|--------|");
  for (const b of buckets) {
    const c = countByStatus(b.leaves);
    const tracking = [...b.trackingNumbers].map((n) => `#${n}`).join(" ");
    lines.push(
      `| ${b.name} | ${tracking} | ${b.leaves.length} | ${c["no-pr"]} | ${c["pr-open"]} | ${c["pr-merged"]} | ${c.blocked} | ${latestForConnector(b.leaves)} |`,
    );
  }
  lines.push("");
  lines.push("## Backlog priority (FIFO by leaf createdAt)");
  const backlog = leaves
    .filter((l) => deriveStatus(l) === "no-pr")
    .sort((a, b) => (a.createdAt < b.createdAt ? -1 : 1))
    .slice(0, 50);
  if (backlog.length === 0) {
    lines.push("_(empty)_");
  } else {
    let i = 1;
    for (const l of backlog) {
      lines.push(`${i}. ${l.connector} #${l.number} (${l.flow}, ${l.createdAt.slice(0, 10)})`);
      i++;
    }
  }
  lines.push("");
  return lines.join("\n");
}

export function renderConnectorMarkdown(name: string, leaves: Leaf[]): string {
  const lines: string[] = [];
  const tracking = [...new Set(leaves.map((l) => l.parentTracking))].map((n) => `#${n}`).join(" ");
  lines.push(`# ${name} — tracking ${tracking || "(none)"}`);
  lines.push(`Last refresh: ${nowIso()}`);
  lines.push("");
  lines.push("## Leaves");
  lines.push("| Issue | Flow | Title | Status | PR | Created |");
  lines.push("|-------|------|-------|--------|----|---------|");
  const sorted = [...leaves].sort((a, b) => a.number - b.number);
  for (const l of sorted) {
    const status = deriveStatus(l);
    const pr = l.linkedPRs[0];
    const prCell = pr ? `${pr.repo}#${pr.number} (${pr.state})` : "—";
    lines.push(`| #${l.number} | ${l.flow} | ${l.title.replace(/\|/g, "\\|").slice(0, 80)} | ${status} | ${prCell} | ${l.createdAt.slice(0, 10)} |`);
  }
  lines.push("");
  const escalations = leaves.filter((l) => l.labels.includes("parity:blocked"));
  if (escalations.length > 0) {
    lines.push("## Escalations open");
    for (const l of escalations) lines.push(`- #${l.number} — see issue for last escalation comment`);
    lines.push("");
  }
  return lines.join("\n");
}

export async function writeDashboardFiles(workspaceRoot: string, leaves: Leaf[], cfg: ParityConfig): Promise<void> {
  await writeAtomic(join(workspaceRoot, "parity-dashboard.md"), renderDashboardMarkdown(leaves, cfg));
  const buckets = group(leaves);
  for (const b of buckets) {
    await writeAtomic(join(workspaceRoot, "connectors", `${b.name}.md`), renderConnectorMarkdown(b.name, b.leaves));
  }
}
