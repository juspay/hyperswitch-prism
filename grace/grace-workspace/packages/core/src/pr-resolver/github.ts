import { execa } from "execa";
import type {
  PRInfo,
  ReviewComment,
  ReviewThread,
  TriggeredThread,
} from "./types.js";

/**
 * GraphQL client for the PR Resolver. Shells out to the `gh` CLI so we
 * inherit whatever auth the user already configured with `gh auth login`
 * — no token plumbing in this codebase. Matches the Python service's
 * approach in `grace/services/pr_resolver/github.py`.
 */

const FETCH_PRS_QUERY = `
query($owner: String!, $repo: String!, $cursor: String) {
  repository(owner: $owner, name: $repo) {
    pullRequests(states: OPEN, first: 50, after: $cursor, orderBy: {field: UPDATED_AT, direction: DESC}) {
      pageInfo { hasNextPage endCursor }
      nodes {
        number
        title
        body
        headRefName
        state
        author { login }
        comments(first: 100) {
          nodes {
            id
            body
            author { login }
            authorAssociation
            createdAt
          }
        }
        reviewThreads(first: 100) {
          nodes {
            id
            isResolved
            isOutdated
            path
            line
            startLine
            comments(first: 50) {
              nodes {
                id
                body
                author { login }
                authorAssociation
                createdAt
                updatedAt
                diffHunk
              }
            }
          }
        }
      }
    }
  }
}
`;

const FETCH_SINGLE_PR_QUERY = `
query($owner: String!, $repo: String!, $number: Int!) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      number
      title
      body
      headRefName
      state
      author { login }
      comments(first: 100) {
        nodes {
          id
          body
          author { login }
          authorAssociation
          createdAt
        }
      }
      reviewThreads(first: 100) {
        nodes {
          id
          isResolved
          isOutdated
          path
          line
          startLine
          comments(first: 50) {
            nodes {
              id
              body
              author { login }
              authorAssociation
              createdAt
              updatedAt
              diffHunk
            }
          }
        }
      }
    }
  }
}
`;

const POST_REPLY_MUTATION = `
mutation($threadId: ID!, $body: String!) {
  addPullRequestReviewThreadReply(input: {pullRequestReviewThreadId: $threadId, body: $body}) {
    comment { id }
  }
}
`;

const ADD_REACTION_MUTATION = `
mutation($subjectId: ID!, $content: ReactionContent!) {
  addReaction(input: {subjectId: $subjectId, content: $content}) {
    reaction { content }
  }
}
`;

interface GraphQLAuthor {
  login?: string;
}

interface GraphQLCommentNode {
  id: string;
  body?: string;
  author?: GraphQLAuthor | null;
  authorAssociation?: string;
  createdAt?: string;
  updatedAt?: string;
  diffHunk?: string;
}

interface GraphQLThreadNode {
  id: string;
  isResolved?: boolean;
  isOutdated?: boolean;
  path?: string;
  line?: number | null;
  startLine?: number | null;
  comments?: { nodes?: GraphQLCommentNode[] };
}

interface GraphQLIssueCommentNode {
  id: string;
  body?: string;
  author?: GraphQLAuthor | null;
  authorAssociation?: string;
  createdAt?: string;
}

interface GraphQLPrNode {
  number: number;
  title?: string;
  body?: string;
  headRefName?: string;
  state?: string;
  author?: GraphQLAuthor | null;
  comments?: { nodes?: GraphQLIssueCommentNode[] };
  reviewThreads?: { nodes?: GraphQLThreadNode[] };
}

interface GraphQLResponse<T> {
  data?: T;
  errors?: Array<{ message: string }>;
}

async function runGraphQL<T>(
  query: string,
  variables: Record<string, string | number>
): Promise<GraphQLResponse<T>> {
  const args = ["api", "graphql", "-f", `query=${query}`];
  for (const [key, value] of Object.entries(variables)) {
    if (typeof value === "number") {
      args.push("-F", `${key}=${value}`);
    } else {
      args.push("-f", `${key}=${value}`);
    }
  }
  const result = await execa("gh", args, {
    reject: false,
    timeout: 30_000,
  });
  if (result.exitCode !== 0) {
    throw new Error(
      `gh api graphql failed (rc=${result.exitCode}): ${result.stderr}`
    );
  }
  const parsed = JSON.parse(result.stdout) as GraphQLResponse<T>;
  if (parsed.errors && parsed.errors.length > 0) {
    const messages = parsed.errors.map((e) => e.message).join("; ");
    throw new Error(`gh api graphql returned errors: ${messages}`);
  }
  return parsed;
}

function parsePrNode(node: GraphQLPrNode): PRInfo {
  const threads: ReviewThread[] = [];
  for (const t of node.reviewThreads?.nodes ?? []) {
    const comments: ReviewComment[] = [];
    for (const c of t.comments?.nodes ?? []) {
      comments.push({
        id: c.id,
        body: c.body ?? "",
        author: c.author?.login ?? "unknown",
        authorAssociation: c.authorAssociation ?? "NONE",
        createdAt: c.createdAt ?? "",
        updatedAt: c.updatedAt ?? "",
        diffHunk: c.diffHunk ?? "",
      });
    }
    threads.push({
      id: t.id,
      isResolved: t.isResolved ?? false,
      isOutdated: t.isOutdated ?? false,
      path: t.path ?? "",
      line: t.line ?? null,
      startLine: t.startLine ?? null,
      comments,
    });
  }
  const issueComments = (node.comments?.nodes ?? []).map((c) => ({
    id: c.id,
    body: c.body ?? "",
    author: c.author?.login ?? "unknown",
    authorAssociation: c.authorAssociation ?? "NONE",
    createdAt: c.createdAt ?? "",
  }));
  return {
    number: node.number,
    title: node.title ?? "",
    body: node.body ?? "",
    headRef: node.headRefName ?? "",
    state: node.state ?? "",
    author: node.author?.login ?? "unknown",
    threads,
    issueComments,
  };
}

export class GitHubClient {
  constructor(
    private readonly owner: string,
    private readonly repo: string
  ) {}

  /** Fetch all OPEN PRs with their review threads, paginated. */
  async fetchOpenPrsWithThreads(): Promise<PRInfo[]> {
    const prs: PRInfo[] = [];
    let cursor: string | null = null;
    // Safety bound — 50 pages * 50 PRs = 2500 open PRs is plenty.
    for (let page = 0; page < 50; page++) {
      const variables: Record<string, string> = {
        owner: this.owner,
        repo: this.repo,
      };
      if (cursor) variables.cursor = cursor;
      const response = await runGraphQL<{
        repository?: {
          pullRequests?: {
            pageInfo?: { hasNextPage?: boolean; endCursor?: string };
            nodes?: GraphQLPrNode[];
          };
        };
      }>(FETCH_PRS_QUERY, variables);
      const conn = response.data?.repository?.pullRequests;
      if (!conn) break;
      for (const node of conn.nodes ?? []) prs.push(parsePrNode(node));
      const pageInfo = conn.pageInfo;
      if (pageInfo?.hasNextPage && pageInfo.endCursor) {
        cursor = pageInfo.endCursor;
      } else {
        break;
      }
    }
    return prs;
  }

  /** Fetch a single PR by number, returning null if it doesn't exist. */
  async fetchPrThreads(prNumber: number): Promise<PRInfo | null> {
    const response = await runGraphQL<{
      repository?: { pullRequest?: GraphQLPrNode | null };
    }>(FETCH_SINGLE_PR_QUERY, {
      owner: this.owner,
      repo: this.repo,
      number: prNumber,
    });
    const node = response.data?.repository?.pullRequest;
    if (!node) return null;
    return parsePrNode(node);
  }

  /** Post a reply to a review thread. Returns true on success. */
  async postThreadReply(threadId: string, body: string): Promise<boolean> {
    try {
      await runGraphQL(POST_REPLY_MUTATION, { threadId, body });
      return true;
    } catch (err) {
      // eslint-disable-next-line no-console
      console.error(
        `[pr-resolver:github] Failed to post reply to thread ${threadId}:`,
        err
      );
      return false;
    }
  }

  /** Add a reaction (default 👀 EYES) to a comment to signal it's been picked up. */
  async addReaction(
    commentNodeId: string,
    reaction = "EYES"
  ): Promise<boolean> {
    try {
      await runGraphQL(ADD_REACTION_MUTATION, {
        subjectId: commentNodeId,
        content: reaction,
      });
      return true;
    } catch {
      return false;
    }
  }
}

/**
 * Return threads that contain the trigger tag and represent fresh work.
 *
 * Trigger match is case-insensitive. If the tag appears in multiple comments
 * in the same thread, we use the **latest** one — earlier triggers were
 * either already processed (and the new trigger is a follow-up) or are
 * superseded by the more recent ask. Path/line come from the thread.
 *
 * Dedup is **per trigger comment id**, not per thread, so a fresh
 * `@trigger` reply in an already-resolved thread surfaces as new work.
 * A `legacyThreadIds` fallback handles state.json entries that pre-date
 * the per-trigger-comment schema (they get one final per-thread skip
 * until they're re-processed under the new code path).
 *
 * `isOutdated` is NOT a hard skip; we propagate it on the TriggeredThread
 * so the resolver prompt can warn Claude that line anchors may have moved.
 */
export function filterTriggeredThreads(
  pr: PRInfo,
  trigger: string,
  processed: {
    triggerCommentIds: Set<string>;
    legacyThreadProcessedAt: Map<string, number>;
  }
): TriggeredThread[] {
  const triggerLower = trigger.toLowerCase();
  const triggerRegex = new RegExp(escapeRegex(trigger), "gi");
  const out: TriggeredThread[] = [];

  for (const thread of pr.threads) {
    if (thread.isResolved) continue;

    // Find the LATEST trigger comment in the thread (was: first/break).
    // GitHub returns comments in chronological order, so the last match
    // is the most recent ask.
    let triggerComment: ReviewComment | undefined;
    for (const comment of thread.comments) {
      if (comment.body.toLowerCase().includes(triggerLower)) {
        triggerComment = comment;
      }
    }
    if (!triggerComment) continue;

    // Skip if we already processed THIS specific trigger comment.
    if (processed.triggerCommentIds.has(triggerComment.id)) continue;
    // Back-compat: legacy state.json entries without a stored trigger id
    // — compare the trigger comment's createdAt against the entry's
    // processed_at. A NEWER trigger comment means the user added a fresh
    // ask after the last resolve; we should pick it up. An older-or-equal
    // comment is the same one we already handled; skip.
    const legacyProcessedAt = processed.legacyThreadProcessedAt.get(thread.id);
    if (legacyProcessedAt !== undefined) {
      const triggerAt = Date.parse(triggerComment.createdAt);
      if (!Number.isFinite(triggerAt) || triggerAt <= legacyProcessedAt) {
        continue;
      }
    }

    const instruction = triggerComment.body.replace(triggerRegex, "").trim();
    const root = thread.comments[0];
    const originalCommentBody = root?.body ?? "";
    const originalAuthor = root?.author ?? "unknown";
    const threadTranscript = thread.comments
      .map((c) => `@${c.author}: ${c.body}`)
      .join("\n---\n");
    const diffHunk =
      triggerComment.diffHunk || (thread.comments[0]?.diffHunk ?? "");

    out.push({
      threadId: thread.id,
      prNumber: pr.number,
      prBranch: pr.headRef,
      path: thread.path,
      line: thread.line,
      instruction,
      author: triggerComment.author,
      originalCommentBody,
      originalAuthor,
      threadTranscript,
      diffHunk,
      commentNodeId: triggerComment.id,
      authorAssociation: triggerComment.authorAssociation,
      isOutdated: thread.isOutdated,
    });
  }

  return out;
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Parse an owner/name string. Throws if the input isn't shaped like
 * `owner/repo` so misconfiguration surfaces early at boot, not during a
 * poll cycle.
 */
export function parseGithubRepo(repo: string): { owner: string; repo: string } {
  const parts = repo.split("/").filter(Boolean);
  if (parts.length !== 2) {
    throw new Error(
      `prResolver.githubRepo must be 'owner/name' format, got: ${repo}`
    );
  }
  return { owner: parts[0]!, repo: parts[1]! };
}
