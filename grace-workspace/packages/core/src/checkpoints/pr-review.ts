import path from "node:path";
import { promises as fs } from "node:fs";
import type { Checkpoint, PRReviewResult } from "../types.js";
import { runAI } from "../tools/runner-factory.js";
import {
  deriveClaudeSessionId,
  friendlySessionName,
} from "./session-id.js";
import { safeParseJson } from "../utils.js";
import { getConfig } from "../config.js";
import { askYesNo } from "../prompts/cli-prompts.js";

const SYSTEM = `You are a senior engineer performing a spec-compliance code review on a hyperswitch-prism PR AND raising the PR.

## Tool Access

You have FULL ACCESS to all tools including Read, Edit, Write, Bash, Grep, Glob, and WebFetch. Use any tool necessary to commit, push, and open the PR.

## Workflow Compliance

STRICTLY FOLLOW the workflow defined in (relative to the per-session worktree):
- Local: {WORKFLOW_PATH}

Additional checks:
- Branch naming: the run-scoped branch is provided in \`userPayload.branch\` (pattern: \`feat/grace-{connector}-{flow}-{runId6}\`, where the trailing 6-hex is the last six chars of the engine's runId). Preflight already created and checked out this branch — verify with \`git branch --show-current\`. **Do NOT create a new branch and do NOT cherry-pick onto a fresh branch off origin/main.** Commit and push directly on the preflight branch.
- If \`git branch --show-current\` does not match \`userPayload.branch\`, run \`git checkout {userPayload.branch}\` (it should already exist locally from preflight).

## Output contract

Return ONLY valid JSON in this exact shape — no STATUS:/PR_URL: free-text format. The engine parses ONLY this JSON:

{
  "approved": boolean,
  "specComplianceScore": number (0..1),
  "comments": [{ "file": "string", "line": number | null, "comment": "string", "severity": "info"|"warning"|"blocking" }],
  "status": "SUCCESS" | "FAILED",
  "prUrl": "https://github.com/juspay/hyperswitch-prism/pull/<n>" | "",
  "branchName": "feat/grace-<connector>-<flow>-<runId6>" | "",
  "commitHash": "<sha>" | "",
  "reason": "<short explanation if status=FAILED, empty string if SUCCESS>"
}

When the workflow asks you to "Output STATUS / PR_URL / PR_BRANCH / REASON", translate those into the corresponding JSON fields above. Do NOT also emit the STATUS-format text — return ONLY the JSON. \`prUrl\` should be the URL captured from the \`gh pr create\` output (or from \`gh pr list\` if the PR already existed when you started).`;

const COLOR: Record<string, string> = {
  info: "\x1b[36m",
  warning: "\x1b[33m",
  blocking: "\x1b[31m",
};

export const prReviewCheckpoint: Checkpoint = {
  id: "pr_review",
  name: "PR review (automated + human gate)",
  description:
    "LLM reviews for spec compliance; human confirms on non-approved output.",
  retryFrom: "pr_review",
  timeout: 25 * 60 * 1000,
  async run(ctx) {
    const implementation = ctx.artifacts.implementation;
    if (!implementation)
      return { passed: false, errors: ["Missing implementation result"] };

    const files: Array<{ path: string; contents: string }> = [];
    const seen = new Set<string>();

    // Get files from implementation result
    const implFiles =
      (implementation as any).filesModified ??
      (implementation as any).files?.map((f: any) => f.path) ??
      [];
    const touched = [...implFiles];
    if (touched.length === 0) {
      return { passed: false, errors: ["No files found in implementation"] };
    }
    ctx.log("[pr_review] Following Grace workflow: 2.4_pr.md", "info");

    for (const rel of touched) {
      if (seen.has(rel)) continue;
      seen.add(rel);
      const abs = path.isAbsolute(rel)
        ? rel
        : path.join(ctx.task.projectRoot, rel);
      try {
        const contents = await fs.readFile(abs, "utf-8");
        files.push({ path: rel, contents: contents.slice(0, 8000) });
      } catch {
        /* ignore */
      }
    }

    let parsed: PRReviewResult;
    // Phase 12: persistent pr_review session. On the first call the agent opens
    // the PR; on retries (e.g. reviewer asks for an amend) it must NOT open a
    // new PR — it amends the existing one via `git commit --amend && git push
    // --force-with-lease`. The resume prompt makes that contract explicit.
    const prSessionId = ctx.artifacts.prReviewSessionId as string | undefined;
    const priorPrReview = ctx.artifacts.prReview as
      | { prUrl?: string }
      | undefined;
    // Phase 15: deterministic session id from (connector, flow, phase).
    const prConnector =
      (ctx.task.targetConnectors && ctx.task.targetConnectors[0]) || "unknown";
    const prFlow = ctx.task.paymentMethod || "unknown";
    const prFriendly = friendlySessionName(prConnector, prFlow, "prreview");
    const prDerived = deriveClaudeSessionId(prConnector, prFlow, "prreview");

    try {
      const aiCall = prSessionId
        ? {
            claudeSessionId: prSessionId,
            incremental: true,
            userPayload:
              `Retry of PR review for branch ${ctx.artifacts.branch ?? "(unknown branch)"}.\n\n` +
              `IMPORTANT: a PR has already been opened${priorPrReview?.prUrl ? ` at ${priorPrReview.prUrl}` : ""}.\n` +
              `Do NOT create a new PR. To incorporate changes, amend the existing PR by:\n` +
              `  1. Make any necessary edits to source files\n` +
              `  2. \`git add\` the changes\n` +
              `  3. \`git commit --amend --no-edit\` (or with a new message if the description needs updating)\n` +
              `  4. \`git push --force-with-lease\` to update the existing PR\n\n` +
              `Then re-run your review and reply with the updated PRReviewResult JSON (same shape as before).`,
            skillBody: "",
          }
        : {
            skillBody: SYSTEM.replace(
              "{WORKFLOW_PATH}",
              path.join(ctx.task.projectRoot, "grace/workflow/2.4_pr.md"),
            ),
            userPayload: {
              l2: ctx.artifacts.l2,
              l3: ctx.artifacts.l3,
              implementation: ctx.artifacts.implementation,
              grpcTest: ctx.artifacts.grpcTest,
              grpcurlOutput: ctx.artifacts.grpcurlOutput,
              branch: ctx.artifacts.branch,
              files,
              cypressReport: ctx.artifacts.cypressReport,
              playwrightReport: ctx.artifacts.playwrightReport,
            },
            preferredSessionId: prDerived,
          };

      const { result: rawParsed, sessionId: nextPrSessionId } =
        await runAI<PRReviewResult>({
          ...aiCall,
          cwd: ctx.task.projectRoot,
          label: prSessionId ? "pr_review:resume" : "pr_review",
          timeoutMs: 20 * 60 * 1000,
          allowWrite: true, // Grant full tool access to PR review agent
          sessionLabel: prFriendly,
        });
      parsed = rawParsed;
      ctx.artifacts.prReviewSessionId = nextPrSessionId;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      ctx.log(`[pr_review] AI runner failed: ${msg}`, "error");
      return {
        passed: false,
        errors: [`PR review AI call failed: ${msg}`],
      };
    }

    if (
      !parsed ||
      typeof parsed.approved !== "boolean" ||
      !Array.isArray(parsed.comments)
    ) {
      return {
        passed: false,
        errors: [
          `Invalid PR review JSON: ${JSON.stringify(parsed).slice(0, 300)}`,
        ],
      };
    }

    // Derive prNumber from prUrl so the dashboard doesn't have to re-parse.
    if (parsed.prUrl && typeof parsed.prUrl === "string") {
      const m = parsed.prUrl.match(/\/pull\/(\d+)/);
      if (m) parsed.prNumber = Number(m[1]);
    }

    // Soft-warn if a SUCCESS run didn't capture a PR URL — don't reject,
    // since the workflow's Phase 1a allows skipping when there are no
    // connector files to commit, but make it visible in the engine log.
    if (parsed.status === "SUCCESS" && !parsed.prUrl) {
      ctx.log(
        "[pr_review] status=SUCCESS but prUrl is empty — agent may have skipped PR creation; check workflow Phase 1a/6a",
        "warn"
      );
    }

    for (const c of parsed.comments) {
      const color = COLOR[c.severity] ?? "";
      // eslint-disable-next-line no-console
      console.log(
        `${color}[${c.severity.toUpperCase()}]\x1b[0m ${c.file}${c.line ? `:${c.line}` : ""} — ${c.comment}`,
      );
    }

    // Lowered threshold for more permissive reviews
    const scoreOk = parsed.specComplianceScore >= 0.6;
    // Auto-pass if approved and score is ok
    const autoPass = parsed.approved && scoreOk;

    // Phase 15: belt-and-suspenders echo (see l2-planning for rationale).
    // Used in all four success/failure returns below.
    const prSessionEcho = { prReviewSessionId: ctx.artifacts.prReviewSessionId };

    if (autoPass) {
      return { passed: true, artifacts: { prReview: parsed, ...prSessionEcho } };
    }

    // If autoApproveReviews is enabled, pass despite issues
    if (ctx.options.autoApproveReviews) {
      ctx.log("[pr_review] Auto-approving despite issues (autoApproveReviews enabled)", "warn");
      return { passed: true, artifacts: { prReview: parsed, ...prSessionEcho } };
    }

    const cfg = getConfig().checkpoints.pr_review;
    // If human approval is not required, pass through to allow pushing changes
    if (!cfg.requireHumanApproval) {
      ctx.log("[pr_review] Passing without human approval (requireHumanApproval: false)", "warn");
      return { passed: true, artifacts: { prReview: parsed, ...prSessionEcho } };
    }

    const timeoutMs = cfg.humanApprovalTimeoutMs;
    const promptP = askYesNo(
      `Manual override: approve this PR despite the review? (score=${parsed.specComplianceScore})`,
      false,
    );
    const timeoutP = new Promise<boolean>((resolve) =>
      setTimeout(() => resolve(false), timeoutMs),
    );
    const approved = await Promise.race([promptP, timeoutP]);
    if (approved) {
      return {
        passed: true,
        artifacts: {
          prReview: { ...parsed, approved: true },
          ...prSessionEcho,
        },
      };
    }
    return {
      passed: false,
      errors: ["Human reviewer rejected PR review"],
      artifacts: { prReview: parsed, ...prSessionEcho },
    };
  },
};
