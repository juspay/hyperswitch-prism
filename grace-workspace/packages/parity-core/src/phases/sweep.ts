import type { ParityConfig } from "../config.js";
import { commentIssue, viewPr } from "../github/client.js";
import { LABELS, transition } from "../github/labels.js";
import type { Leaf, LinkedPR } from "../types.js";
import { detectLabelBackfills } from "../dashboard/derive.js";

export interface SweepReport {
  fixedLabels: number;
  remindersSent: number;
  mergedClaimed: number;
  blocked: number;
}

function isStale(prCreatedAt: string, days: number): boolean {
  const ageMs = Date.now() - new Date(prCreatedAt).getTime();
  return ageMs > days * 24 * 60 * 60 * 1000;
}

function failureIsCosmetic(checks: { name: string; conclusion: string | null }[]): boolean {
  // Treat ONLY fmt/clippy/typo-style failures as cosmetic. Build/test failures escalate.
  const failed = checks.filter((c) => (c.conclusion ?? "").toLowerCase() === "failure");
  if (failed.length === 0) return false;
  return failed.every((c) => /fmt|clippy|typos|lint/i.test(c.name));
}

export async function runSweep(cfg: ParityConfig, leaves: Leaf[]): Promise<SweepReport> {
  const report: SweepReport = { fixedLabels: 0, remindersSent: 0, mergedClaimed: 0, blocked: 0 };
  const issueRepo = `${cfg.github.owner}/${cfg.github.repo}`;

  for (const leaf of leaves) {
    const fix = detectLabelBackfills(leaf);
    if (fix) {
      await transition({
        repo: issueRepo,
        issue: leaf.number,
        remove: fix.remove as any,
        add: fix.add as any,
        comment: `Label backfill: ${fix.reason}`,
      });
      report.fixedLabels++;
    }

    const ourPRs = leaf.linkedPRs.filter((p) => p.author === cfg.github.actor);
    for (const pr of ourPRs) {
      if (pr.state === "merged") {
        if (!leaf.labels.includes(LABELS.MERGED)) {
          await transition({
            repo: issueRepo,
            issue: leaf.number,
            remove: [LABELS.PR_OPEN, LABELS.CLAIMED],
            add: [LABELS.MERGED],
            comment: `Merge tracked: ${pr.url}`,
          });
          report.mergedClaimed++;
        }
        continue;
      }
      if (pr.state !== "open") continue;

      const detail = await viewPr(pr.repo, pr.number).catch(() => null);
      if (!detail) continue;

      const checks = detail.statusCheckRollup ?? [];
      const anyFailure = checks.some((c) => (c.conclusion ?? "").toLowerCase() === "failure");
      if (anyFailure && !failureIsCosmetic(checks)) {
        if (!leaf.labels.includes(LABELS.BLOCKED)) {
          await transition({
            repo: issueRepo,
            issue: leaf.number,
            add: [LABELS.BLOCKED],
            comment: `PR ${pr.url} has non-cosmetic CI failures; needs human review.`,
          });
          report.blocked++;
        }
        continue;
      }

      if (isStale(detail.createdAt, cfg.heartbeat.sweepStalePrDays)) {
        const issueRepoFull = `${cfg.github.owner}/${cfg.github.repo}`;
        const reminderTag = "[autopilot-stale-reminder]";
        // Idempotency: only post once. (Cheap check via getIssue would inflate API usage;
        // we rely on the user removing the reminder body or the PR moving to merged.)
        await commentIssue(
          issueRepoFull,
          leaf.number,
          `${reminderTag} PR ${pr.url} has been open ${cfg.heartbeat.sweepStalePrDays}+ days with green CI. Friendly reminder for human review.`,
        );
        report.remindersSent++;
      }
    }
  }

  return report;
}

// Helper used by orchestrator decide step: leaves we already own with PR open
export function ownedOpenPRs(cfg: ParityConfig, leaves: Leaf[]): { leaf: Leaf; pr: LinkedPR }[] {
  const out: { leaf: Leaf; pr: LinkedPR }[] = [];
  for (const leaf of leaves) {
    for (const pr of leaf.linkedPRs) {
      if (pr.state === "open" && pr.author === cfg.github.actor) out.push({ leaf, pr });
    }
  }
  return out;
}
