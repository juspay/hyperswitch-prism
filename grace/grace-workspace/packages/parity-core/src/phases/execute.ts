import { execa, type ExecaError } from "execa";
import { runAI, withRunner, type RunnerType } from "@10xgrace/core";
import type { ParityConfig } from "../config.js";
import { EXECUTE_SYSTEM, buildExecuteUser } from "../prompts/execute.js";
import { branchName, classifyTarget, validateDiff } from "../locus.js";
import type { ExecuteResult, Leaf, Locus } from "../types.js";

async function git(repoRoot: string, args: string[]): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  try {
    const r = await execa("git", args, { cwd: repoRoot });
    return { stdout: r.stdout ?? "", stderr: r.stderr ?? "", exitCode: r.exitCode ?? 0 };
  } catch (err) {
    const e = err as ExecaError;
    return { stdout: e.stdout?.toString() ?? "", stderr: e.stderr?.toString() ?? "", exitCode: e.exitCode ?? 1 };
  }
}

async function cargoCmd(repoRoot: string, args: string[]): Promise<{ ok: boolean; tail: string }> {
  try {
    const r = await execa("cargo", args, { cwd: repoRoot, all: true, reject: false });
    const all = (r as any).all ?? r.stdout ?? "";
    const tail = String(all).split("\n").slice(-50).join("\n");
    return { ok: (r.exitCode ?? 1) === 0, tail };
  } catch (err) {
    const e = err as ExecaError;
    const tail = String(e.stdout?.toString() ?? e.stderr?.toString() ?? "").split("\n").slice(-50).join("\n");
    return { ok: false, tail };
  }
}

function pkgForTarget(target: "prism" | "hyperswitch-bridge"): string {
  return target === "prism" ? "connector-integration" : "external_services";
}

export async function runExecute(opts: {
  cfg: ParityConfig;
  leaf: Leaf;
  planMarkdown: string;
  understoodLocus: Locus;
  declaredTarget?: "prism" | "hyperswitch-bridge";
  sessionId?: string;
}): Promise<ExecuteResult & { sessionId?: string }> {
  const cls = classifyTarget({
    declaredTarget: opts.declaredTarget,
    understoodLocus: opts.understoodLocus,
    bridgeAvailable: !!opts.cfg.bridgeWritePath,
  });

  if (cls.target === "escalate") {
    return {
      ok: false,
      target: "escalate",
      repoRoot: "",
      branch: "",
      changedFiles: [],
      buildTail: "",
      clippyTail: "",
      testTail: "",
      escalation: { blocker: cls.reason ?? "locus rejected by bridge gate", tried: "classifyTarget()", question: "How should this leaf proceed?" },
    };
  }

  const repoRoot = cls.target === "prism" ? opts.cfg.prismPath : opts.cfg.bridgeWritePath;
  if (!repoRoot) {
    return { ok: false, target: cls.target, repoRoot: "", branch: "", changedFiles: [], buildTail: "", clippyTail: "", testTail: "",
      escalation: { blocker: `repo root for target ${cls.target} is empty`, tried: "config lookup", question: "Set PARITY_BRIDGE_PATH or fix config." } };
  }

  // Narrow cls.target now so the lambda below can capture it. After the
  // earlier `if (cls.target === "escalate") return ...` guard, the only
  // remaining cases are "prism" and "hyperswitch-bridge".
  const target: "prism" | "hyperswitch-bridge" = cls.target as "prism" | "hyperswitch-bridge";

  await git(repoRoot, ["fetch", "origin"]);
  const branch = branchName(target, opts.leaf.connector, opts.leaf.flow, opts.leaf.title);
  const exists = await git(repoRoot, ["rev-parse", "--verify", branch]);
  if (exists.exitCode === 0) {
    await git(repoRoot, ["checkout", branch]);
  } else {
    await git(repoRoot, ["checkout", "-b", branch, "origin/main"]);
  }

  const effective: RunnerType = opts.cfg.llm.runner ?? "claude-code";
  const ai = await withRunner(effective, () =>
    runAI<string>({
      skillBody: EXECUTE_SYSTEM,
      userPayload: buildExecuteUser(opts.planMarkdown, repoRoot, target),
      cwd: repoRoot,
      label: `parity/execute/${opts.leaf.number}`,
      model: opts.cfg.llm.executeModel || undefined,
      rawText: true,
      allowWrite: true,
      // EXECUTE runs cargo + edits; bridge-target executes can take >10 min.
      // Cancellation is still available via the dashboard's "Cancel run (SIGTERM)" button.
      timeoutMs: 0,
      ...(effective === "claude-code"
        ? {
            claudeSessionId: opts.sessionId,
            sessionLabel: `parity-${opts.leaf.connector}-${opts.leaf.flow}-execute`,
          }
        : {}),
    }),
  );

  const diff = await git(repoRoot, ["diff", "--name-only", "HEAD"]);
  const stagedDiff = await git(repoRoot, ["diff", "--name-only", "--cached"]);
  const changedFiles = [...new Set((diff.stdout + "\n" + stagedDiff.stdout).split("\n").map((s) => s.trim()).filter(Boolean))];

  const v = validateDiff({ changedFiles, target, rules: opts.cfg.rules });
  if (!v.ok) {
    return { ok: false, target, repoRoot, branch, changedFiles, buildTail: "", clippyTail: "", testTail: "",
      escalation: { blocker: "Forbidden surface touched", tried: "validateDiff", question: v.violations.join("; ") } };
  }
  if (changedFiles.length === 0) {
    return { ok: false, target, repoRoot, branch, changedFiles, buildTail: "", clippyTail: "", testTail: "",
      escalation: { blocker: "Execute produced no diff", tried: "git diff", question: "Why did the agent report apply but no files changed?" } };
  }

  const pkg = pkgForTarget(target);
  const build = await cargoCmd(repoRoot, ["build", "-p", pkg]);
  const clippy = await cargoCmd(repoRoot, ["clippy", "-p", pkg, "--", "-D", "warnings"]);
  const tests = await cargoCmd(repoRoot, ["nextest", "run", "-p", pkg]);

  void ai.result;
  return {
    ok: build.ok && clippy.ok && tests.ok,
    target,
    repoRoot,
    branch,
    changedFiles,
    buildTail: build.tail,
    clippyTail: clippy.tail,
    testTail: tests.tail,
    sessionId: ai.sessionId,
    ...(build.ok && clippy.ok && tests.ok ? {} : { escalation: { blocker: "cargo failed", tried: "build/clippy/nextest", question: "See tails." } }),
  };
}
