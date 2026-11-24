import { execa, type ExecaError } from "execa";
import { credNeedles, describeLeaks, findLeaks, issueNeedles } from "@10xgrace/core";
import type { ParityConfig } from "../config.js";
import { commentIssue, createPrDraft } from "../github/client.js";
import { LABELS, transition } from "../github/labels.js";
import type { ExecuteResult, Leaf, UnderstandingResult, VerifyResult } from "../types.js";

async function git(repoRoot: string, args: string[]): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  try {
    const r = await execa("git", args, { cwd: repoRoot });
    return { stdout: r.stdout ?? "", stderr: r.stderr ?? "", exitCode: r.exitCode ?? 0 };
  } catch (err) {
    const e = err as ExecaError;
    return { stdout: e.stdout?.toString() ?? "", stderr: e.stderr?.toString() ?? "", exitCode: e.exitCode ?? 1 };
  }
}

function targetRepo(target: ExecuteResult["target"]): string {
  if (target === "hyperswitch-bridge") return "juspay/hyperswitch";
  return "juspay/hyperswitch-prism";
}

function renderPrBody(opts: {
  leaf: Leaf;
  understanding: UnderstandingResult;
  planMarkdown: string;
  exec: ExecuteResult;
  verify: VerifyResult;
}): string {
  const { leaf, understanding, planMarkdown, exec, verify } = opts;
  return [
    // Public repo: nothing from the private tracker issue (title, URL, number,
    // merchant, counts). The issue links back to this PR via handoff's comment.
    "## Summary",
    `Parity fix for ${leaf.connector}/${leaf.flow}: align the UCS response with hyperswitch.`,
    "",
    understanding.markdown ?? "",
    "",
    planMarkdown,
    "",
    "## Build tail",
    "```",
    exec.buildTail,
    "```",
    "",
    "## Clippy tail",
    "```",
    exec.clippyTail,
    "```",
    "",
    "## Test results",
    "```",
    exec.testTail,
    "```",
    "",
    verify.markdown,
    "",
    "## Acceptance Criteria",
    "- [x] Build passes",
    "- [x] Clippy passes",
    "- [x] Tests pass",
    "- [x] gRPC verification PASS (target field matches oracle, logs show post-fix code path)",
    "- [ ] Shadow-replay produces zero diff (human verification gate)",
    "",
  ].join("\n");
}

export interface HandoffResult {
  ok: boolean;
  prUrl?: string;
  escalation?: { blocker: string; tried: string; question: string };
}

export async function runHandoff(opts: {
  cfg: ParityConfig;
  leaf: Leaf;
  understanding: UnderstandingResult;
  planMarkdown: string;
  exec: ExecuteResult;
  verify: VerifyResult;
  dryRun?: boolean;
}): Promise<HandoffResult> {
  const { cfg, leaf, exec } = opts;
  if (!exec.ok || exec.target === "escalate") {
    return { ok: false, escalation: { blocker: "Cannot hand off failed execute", tried: "runHandoff guard", question: "Should not reach here." } };
  }

  // Never derived from leaf.title: tracker titles carry the merchant ("[prod] … bu_xx / stripe / …").
  const commitMsg = `parity(${leaf.connector}/${leaf.flow}): align response with hyperswitch`;
  const body = renderPrBody({
    leaf,
    understanding: opts.understanding,
    planMarkdown: opts.planMarkdown,
    exec,
    verify: opts.verify,
  });
  // The PR lands in a public repo, the leaf comes from a private one: fail closed.
  const needles = [...issueNeedles(`${leaf.title}\n${leaf.body}`, [leaf.connector, leaf.flow]), ...credNeedles(cfg.credsPath)];
  const scrubEscalation = (tried: string) => ({
    ok: false,
    escalation: {
      blocker: "public-output scrub gate",
      tried,
      question: "Remove the private identifiers / secrets from the summary, plan or diff, then re-run handoff.",
    },
  });
  const textLeaks = findLeaks(`${commitMsg}\n${body}`, needles);
  if (textLeaks.length > 0) return scrubEscalation(describeLeaks(textLeaks));

  if (opts.dryRun) {
    // Dry-run: agents + cargo + grpc verify all ran. We do NOT commit, push, open a PR,
    // or transition any GitHub label. The execute branch is left checked out in the local
    // prism repo so the operator can inspect the diff before promoting to a live run.
    return { ok: true, prUrl: "(dry-run — no PR created)" };
  }

  const addAll = await git(exec.repoRoot, ["add", "--", ...exec.changedFiles]);
  if (addAll.exitCode !== 0) {
    return { ok: false, escalation: { blocker: "git add failed", tried: addAll.stderr, question: "Inspect repo state." } };
  }

  const staged = await git(exec.repoRoot, ["diff", "--cached"]);
  const diffLeaks = findLeaks(staged.stdout, needles);
  if (staged.exitCode !== 0 || diffLeaks.length > 0) {
    await git(exec.repoRoot, ["reset", "-q", "--", ...exec.changedFiles]);
    return scrubEscalation(staged.exitCode !== 0 ? `git diff --cached failed: ${staged.stderr}` : describeLeaks(diffLeaks));
  }
  const commit = await git(exec.repoRoot, ["commit", "-m", commitMsg]);
  if (commit.exitCode !== 0) {
    return {
      ok: false,
      escalation: {
        blocker: "git commit failed (likely a pre-commit hook). Spec says: fix the cause and create a NEW commit. Do NOT use --no-verify or --amend.",
        tried: commit.stderr || commit.stdout,
        question: "What does the hook require?",
      },
    };
  }
  const push = await git(exec.repoRoot, ["push", "-u", "origin", exec.branch]);
  if (push.exitCode !== 0) {
    return { ok: false, escalation: { blocker: "git push failed", tried: push.stderr, question: "Inspect remote." } };
  }

  const prUrl = await createPrDraft({
    repo: targetRepo(exec.target),
    title: commitMsg,
    body,
    head: exec.branch,
    base: "main",
  });

  await transition({
    repo: `${cfg.github.owner}/${cfg.github.repo}`,
    issue: leaf.number,
    remove: [LABELS.CLAIMED, LABELS.RCA_DONE],
    add: [LABELS.PR_OPEN],
    comment: `Draft PR opened: ${prUrl}`,
  });
  await commentIssue(`${cfg.github.owner}/${cfg.github.repo}`, leaf.number, `gRPC verification: PASS\n\nDraft PR: ${prUrl}`);

  return { ok: true, prUrl };
}
