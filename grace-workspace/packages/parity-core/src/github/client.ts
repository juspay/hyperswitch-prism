import { execa, type ExecaError } from "execa";

export interface GhCallOpts {
  /** Stdin payload (for templates that read body from stdin). */
  input?: string;
  /** Defaults to true; set false to capture even on non-zero exit. */
  rejectOnError?: boolean;
}

async function ghRun(args: string[], opts: GhCallOpts = {}): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  try {
    const res = await execa("gh", args, {
      input: opts.input,
      reject: opts.rejectOnError !== false,
    });
    return { stdout: res.stdout ?? "", stderr: res.stderr ?? "", exitCode: res.exitCode ?? 0 };
  } catch (err) {
    const e = err as ExecaError;
    if (opts.rejectOnError === false) {
      return { stdout: e.stdout?.toString() ?? "", stderr: e.stderr?.toString() ?? "", exitCode: e.exitCode ?? 1 };
    }
    throw err;
  }
}

export async function ghJson<T>(args: string[], opts: GhCallOpts = {}): Promise<T> {
  const { stdout } = await ghRun(args, opts);
  return JSON.parse(stdout) as T;
}

export async function ghGraphQL<T>(query: string, vars: Record<string, string | number>): Promise<T> {
  const args = ["api", "graphql"];
  // gh passes -F for typed scalars; -f for strings. Numbers must use -F.
  for (const [k, v] of Object.entries(vars)) {
    if (typeof v === "number") args.push("-F", `${k}=${v}`);
    else args.push("-F", `${k}=${v}`);
  }
  args.push("-f", `query=${query}`);
  return ghJson<T>(args);
}

export interface GhIssueView {
  number: number;
  title: string;
  body: string;
  state: string;
  url: string;
  createdAt: string;
  labels: { name: string }[];
  author?: { login: string };
  comments?: { author: { login: string }; body: string; createdAt: string }[];
}

export async function getIssue(repo: string, number: number): Promise<GhIssueView> {
  return ghJson<GhIssueView>([
    "issue", "view", String(number),
    "--repo", repo,
    "--json", "number,title,body,state,url,createdAt,labels,author,comments",
  ]);
}

export async function editIssueLabels(repo: string, number: number, addLabels: string[], removeLabels: string[]): Promise<void> {
  if (addLabels.length === 0 && removeLabels.length === 0) return;
  const args = ["issue", "edit", String(number), "--repo", repo];
  for (const l of addLabels) args.push("--add-label", l);
  for (const l of removeLabels) args.push("--remove-label", l);
  await ghRun(args);
}

export async function commentIssue(repo: string, number: number, body: string): Promise<void> {
  if (!body.trim()) throw new Error("commentIssue: refusing to post empty body (audit log requires content)");
  await ghRun(["issue", "comment", String(number), "--repo", repo, "--body-file", "-"], { input: body });
}

export interface GhPrView {
  number: number;
  state: string;
  mergedAt: string | null;
  url: string;
  headRefName: string;
  author: { login: string };
  statusCheckRollup?: { name: string; conclusion: string | null; status: string }[];
  createdAt: string;
}

export async function viewPr(repo: string, number: number): Promise<GhPrView> {
  return ghJson<GhPrView>([
    "pr", "view", String(number),
    "--repo", repo,
    "--json", "number,state,mergedAt,url,headRefName,author,statusCheckRollup,createdAt",
  ]);
}

export async function createPrDraft(opts: {
  repo: string;
  title: string;
  body: string;
  head: string;
  base: string;
}): Promise<string> {
  const args = [
    "pr", "create",
    "--repo", opts.repo,
    "--title", opts.title,
    "--body-file", "-",
    "--head", opts.head,
    "--base", opts.base,
    "--draft",
  ];
  const res = await ghRun(args, { input: opts.body });
  const m = res.stdout.match(/https:\/\/github\.com\/\S+\/pull\/\d+/);
  if (!m) throw new Error(`Could not parse PR URL from gh output: ${res.stdout}`);
  return m[0];
}
