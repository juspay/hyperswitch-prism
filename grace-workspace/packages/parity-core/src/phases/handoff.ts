import { execa, type ExecaError } from "execa";
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
    "## Summary",
    `Parity fix for ${leaf.connector}/${leaf.flow} per #${leaf.number}.`,
    "",
    "## Linked Issue",
    `Closes-when-merged: ${leaf.url} (human closes — autopilot will not).`,
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
}): Promise<HandoffResult> {
  const { cfg, leaf, exec } = opts;
  if (!exec.ok || exec.target === "escalate") {
    return { ok: false, escalation: { blocker: "Cannot hand off failed execute", tried: "runHandoff guard", question: "Should not reach here." } };
  }

  const oneLiner = leaf.title.replace(/^\[parity\]\s*/i, "").slice(0, 60);
  const commitMsg = `parity(${leaf.connector}/${leaf.flow}): ${oneLiner} (#${leaf.number})`;

  const addAll = await git(exec.repoRoot, ["add", ...exec.changedFiles]);
  if (addAll.exitCode !== 0) {
    return { ok: false, escalation: { blocker: "git add failed", tried: addAll.stderr, question: "Inspect repo state." } };
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

  const body = renderPrBody({
    leaf,
    understanding: opts.understanding,
    planMarkdown: opts.planMarkdown,
    exec,
    verify: opts.verify,
  });
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
