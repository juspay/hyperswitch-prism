import { execa } from "execa";
import fs from "node:fs";
import path from "node:path";

/**
 * Thin git helpers around the single working clone used by the PR resolver.
 * Matches the surface of Python `clone_pool.py` for the operations we
 * actually use (clone, fetch, checkout, status, stage, commit, push,
 * revert). Concurrency is out of scope here — `service.ts` serialises
 * calls.
 */

const DEFAULT_GIT_TIMEOUT_MS = 90_000;
const PUSH_TIMEOUT_MS = 180_000;

interface CmdResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

async function run(
  cmd: string,
  args: string[],
  cwd: string,
  timeoutMs = DEFAULT_GIT_TIMEOUT_MS
): Promise<CmdResult> {
  const result = await execa(cmd, args, {
    cwd,
    reject: false,
    timeout: timeoutMs,
  });
  return {
    exitCode: result.exitCode ?? -1,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

/**
 * Ensure the working clone exists. If not, clones `owner/repo` via
 * `gh repo clone` so auth is inherited from the user's `gh` login.
 */
export async function ensureWorktree(input: {
  worktreePath: string;
  owner: string;
  repo: string;
}): Promise<void> {
  if (
    fs.existsSync(input.worktreePath) &&
    fs.existsSync(path.join(input.worktreePath, ".git"))
  ) {
    return;
  }
  fs.mkdirSync(path.dirname(input.worktreePath), { recursive: true });
  const target = path.basename(input.worktreePath);
  const parent = path.dirname(input.worktreePath);
  const result = await execa(
    "gh",
    ["repo", "clone", `${input.owner}/${input.repo}`, target],
    { cwd: parent, reject: false, timeout: 10 * 60 * 1000 }
  );
  if (result.exitCode !== 0) {
    throw new Error(
      `gh repo clone failed (rc=${result.exitCode}): ${result.stderr || result.stdout}`
    );
  }
}

/**
 * Prepare the clone for a fresh PR run: discard any leftover changes from
 * a previous cycle and `gh pr checkout` onto the PR branch.
 *
 * We deliberately do NOT run `git fetch origin` (the broad form). On
 * macOS's case-insensitive APFS, repos with branches that only differ in
 * casing (e.g. `Billdesk-xyne` and `billdesk-xyne`) make the broad fetch
 * refuse to write conflicting refs and exit non-zero. `gh pr checkout`
 * fetches just `refs/pull/<N>/head` (and the branch it lands on),
 * sidestepping the issue entirely.
 */
export async function prepareForPr(input: {
  worktreePath: string;
  prNumber: number;
}): Promise<{ ok: boolean; error?: string }> {
  await run("git", ["reset", "--hard", "HEAD"], input.worktreePath);
  await run("git", ["clean", "-fd"], input.worktreePath);
  const checkout = await run(
    "gh",
    ["pr", "checkout", String(input.prNumber)],
    input.worktreePath
  );
  if (checkout.exitCode !== 0) {
    return { ok: false, error: `gh pr checkout failed: ${checkout.stderr}` };
  }
  return { ok: true };
}

/** Resolve the current HEAD's SHA. */
export async function headSha(worktreePath: string): Promise<string> {
  const result = await run("git", ["rev-parse", "HEAD"], worktreePath);
  return result.stdout.trim();
}

/** Files changed vs HEAD (uncommitted edits Claude made). */
export async function changedFiles(worktreePath: string): Promise<string[]> {
  const result = await run("git", ["diff", "--name-only"], worktreePath);
  if (result.exitCode !== 0) return [];
  return result.stdout
    .split("\n")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/** Discard ALL uncommitted changes — used to roll back a failed sub-task. */
export async function revertAll(worktreePath: string): Promise<void> {
  await run("git", ["checkout", "--", "."], worktreePath);
  await run("git", ["clean", "-fd"], worktreePath);
}

/** `git checkout -- <path>` for a single path. */
export async function revertPath(
  worktreePath: string,
  filePath: string
): Promise<void> {
  await run("git", ["checkout", "--", filePath], worktreePath);
}

/** Stage all changes under `connectors/<connector>` only. */
export async function stageConnector(
  worktreePath: string,
  connector: string
): Promise<{ ok: boolean; error?: string }> {
  // Match the Python service: stage both `<connector>.rs` (the trait impl
  // entrypoint) and the `<connector>/` directory (transformers, types, etc.).
  // `git add --pathspec-from-file=-` would be cleaner but adds nothing for
  // two paths.
  for (const spec of [
    `**/connectors/${connector}.rs`,
    `**/connectors/${connector}/**`,
  ]) {
    const result = await run("git", ["add", spec], worktreePath);
    if (result.exitCode !== 0) {
      // Non-existence of the pattern is fine; only fail on genuine errors.
      const stderr = result.stderr.toLowerCase();
      if (!stderr.includes("did not match")) {
        return { ok: false, error: result.stderr };
      }
    }
  }
  return { ok: true };
}

/** Commit staged changes. Returns the new SHA on success. */
export async function commit(
  worktreePath: string,
  message: string
): Promise<{ ok: boolean; sha?: string; error?: string }> {
  const result = await run(
    "git",
    ["commit", "-m", message],
    worktreePath
  );
  if (result.exitCode !== 0) {
    // Treat "nothing to commit" as a soft failure so the caller can choose
    // whether to surface it as "no changes" vs an actual error.
    const stderr = (result.stderr + result.stdout).toLowerCase();
    if (stderr.includes("nothing to commit")) {
      return { ok: false, error: "nothing to commit" };
    }
    return { ok: false, error: result.stderr || result.stdout };
  }
  const sha = await headSha(worktreePath);
  return { ok: true, sha };
}

/**
 * Push policy: we ONLY fast-forward. Never `--force`, never `--force-with-lease`.
 * If the remote has moved while we were working we rebase our commits on top of
 * it (which keeps our commits as new commits *added* to the remote, never
 * rewritten history). If the rebase fails — typically because someone else
 * edited the same lines we touched — we surface the error and refuse to push.
 *
 * Before every push we verify that `origin/<branch>` is an ancestor of our
 * local HEAD; if it isn't we bail rather than risk a non-fast-forward attempt.
 */
export async function pushBranch(
  worktreePath: string,
  branch: string
): Promise<{ ok: boolean; error?: string }> {
  const firstCheck = await assertFastForwardable(worktreePath, branch);
  if (!firstCheck.ok) {
    return firstCheck;
  }

  let result = await run(
    "git",
    ["push", "origin", branch],
    worktreePath,
    PUSH_TIMEOUT_MS
  );
  if (result.exitCode === 0) return { ok: true };

  // Remote moved between our work and our push attempt. Rebase our commits on
  // top of the new remote tip, re-assert the fast-forward invariant, then
  // push again. No --force needed once the rebase succeeds.
  const rebase = await run(
    "git",
    ["pull", "--rebase", "origin", branch],
    worktreePath,
    PUSH_TIMEOUT_MS
  );
  if (rebase.exitCode !== 0) {
    return {
      ok: false,
      error: `rebase failed (refusing to force-push): ${rebase.stderr || rebase.stdout}`,
    };
  }

  const secondCheck = await assertFastForwardable(worktreePath, branch);
  if (!secondCheck.ok) {
    return secondCheck;
  }

  result = await run(
    "git",
    ["push", "origin", branch],
    worktreePath,
    PUSH_TIMEOUT_MS
  );
  if (result.exitCode !== 0) {
    return {
      ok: false,
      error: `push failed (refusing to force-push): ${result.stderr || result.stdout}`,
    };
  }
  return { ok: true };
}

/**
 * Refuse to push unless `origin/<branch>` is an ancestor of the local HEAD —
 * the explicit invariant that prevents any code path here from silently
 * rewriting remote history. `git merge-base --is-ancestor A B` exits 0 iff
 * A is an ancestor of B.
 */
async function assertFastForwardable(
  worktreePath: string,
  branch: string
): Promise<{ ok: boolean; error?: string }> {
  const remote = `origin/${branch}`;
  // First make sure we have an up-to-date ref for the remote so the ancestor
  // check sees the real tip.
  const fetch = await run(
    "git",
    ["fetch", "origin", branch],
    worktreePath
  );
  if (fetch.exitCode !== 0) {
    return {
      ok: false,
      error: `git fetch origin ${branch} failed: ${fetch.stderr || fetch.stdout}`,
    };
  }
  const ancestry = await run(
    "git",
    ["merge-base", "--is-ancestor", remote, "HEAD"],
    worktreePath
  );
  if (ancestry.exitCode === 0) {
    return { ok: true };
  }
  return {
    ok: false,
    error: `${remote} is not an ancestor of local HEAD — refusing to push (would require --force).`,
  };
}

/** Run `cargo fmt --all` to normalise formatting before commit. */
export async function cargoFmt(
  worktreePath: string
): Promise<{ ok: boolean; output: string }> {
  const result = await run(
    "cargo",
    ["fmt", "--all"],
    worktreePath,
    10 * 60 * 1000
  );
  return {
    ok: result.exitCode === 0,
    output: (result.stderr || result.stdout).slice(-4000),
  };
}

/**
 * Capture the unified diff between `origin/<branch>` and local HEAD —
 * the cumulative change set the user reviews before approving the push.
 * Truncates large diffs to keep the on-disk state file manageable.
 */
export async function capturePrDiff(
  worktreePath: string,
  branch: string,
  maxChars = 80_000
): Promise<string> {
  // Refresh the remote ref so the diff is against the actual tip, not
  // whatever fetch-time stash we had earlier in the cycle.
  await run("git", ["fetch", "origin", branch], worktreePath);
  const result = await run(
    "git",
    ["diff", `origin/${branch}..HEAD`],
    worktreePath
  );
  if (result.exitCode !== 0) return "";
  const diff = result.stdout;
  if (diff.length <= maxChars) return diff;
  return (
    diff.slice(0, maxChars) +
    `\n\n... (truncated; original ${diff.length} chars)`
  );
}

/** Discard any local commits, returning the worktree to origin/<branch>. */
export async function resetToRemote(
  worktreePath: string,
  branch: string
): Promise<{ ok: boolean; error?: string }> {
  await run("git", ["fetch", "origin", branch], worktreePath);
  const result = await run(
    "git",
    ["reset", "--hard", `origin/${branch}`],
    worktreePath
  );
  if (result.exitCode !== 0) {
    return { ok: false, error: result.stderr || result.stdout };
  }
  await run("git", ["clean", "-fd"], worktreePath);
  return { ok: true };
}

/**
 * Use `gh pr view <prNumber>` to look up the PR's current head SHA without
 * touching the working clone. Cheaper than `git ls-remote` because gh
 * already cached the auth.
 */
export async function fetchPrHeadSha(input: {
  worktreePath: string;
  owner: string;
  repo: string;
  prNumber: number;
}): Promise<string> {
  const result = await run(
    "gh",
    [
      "pr",
      "view",
      String(input.prNumber),
      "--repo",
      `${input.owner}/${input.repo}`,
      "--json",
      "headRefOid",
      "--jq",
      ".headRefOid",
    ],
    input.worktreePath,
    30_000
  );
  if (result.exitCode !== 0) return "";
  return result.stdout.trim();
}
