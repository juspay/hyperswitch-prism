import type { ParityConfig } from "../config.js";
import { dailyTreeCachePath, readIfFresh, writeAtomic } from "../cache.js";
import { getIssue, ghGraphQL } from "./client.js";
import type { Leaf, LinkedPR } from "../types.js";
import { join } from "node:path";

interface GqlNode {
  number: number;
  title: string;
  state: string;
  url: string;
  createdAt?: string;
  body?: string;
  labels?: { nodes: { name: string }[] };
  subIssues?: { nodes: GqlNode[] };
}

const TREE_QUERY = `
query($owner:String!,$repo:String!,$num:Int!){
  repository(owner:$owner,name:$repo){
    issue(number:$num){
      number title state url createdAt body
      labels(first:50){ nodes{ name } }
      subIssues(first:100){ nodes { number title state url createdAt } }
    }
  }
}`;

interface GqlRoot {
  data: { repository: { issue: GqlNode } };
}

async function fetchNode(cfg: ParityConfig, num: number): Promise<GqlNode | null> {
  try {
    const root = await ghGraphQL<GqlRoot>(TREE_QUERY, {
      owner: cfg.github.owner,
      repo: cfg.github.repo,
      num,
    });
    return root.data?.repository?.issue ?? null;
  } catch {
    return null;
  }
}

const TASKLIST_PATTERN = /(?:^|\s)(?:- \[[ x]\] )?(?:https?:\/\/github\.com\/([^/]+)\/([^/]+)\/issues\/(\d+)|#(\d+))/g;

function parseTaskListIssueRefs(body: string, defaultOwner: string, defaultRepo: string): { repo: string; number: number }[] {
  const out: { repo: string; number: number }[] = [];
  let m: RegExpExecArray | null;
  while ((m = TASKLIST_PATTERN.exec(body)) !== null) {
    if (m[1] && m[2] && m[3]) {
      out.push({ repo: `${m[1]}/${m[2]}`, number: Number(m[3]) });
    } else if (m[4]) {
      out.push({ repo: `${defaultOwner}/${defaultRepo}`, number: Number(m[4]) });
    }
  }
  return out;
}

const TIMELINE_QUERY = `
query($owner:String!,$repo:String!,$num:Int!){
  repository(owner:$owner,name:$repo){
    issue(number:$num){
      timelineItems(itemTypes:[CROSS_REFERENCED_EVENT],first:50){
        nodes{
          ... on CrossReferencedEvent {
            source {
              ... on PullRequest {
                number state merged mergedAt url
                author{ login }
                repository{ nameWithOwner }
              }
            }
          }
        }
      }
    }
  }
}`;

interface TimelineRoot {
  data: {
    repository: {
      issue: {
        timelineItems: {
          nodes: {
            source?: {
              number?: number;
              state?: string;
              merged?: boolean;
              mergedAt?: string | null;
              url?: string;
              author?: { login: string };
              repository?: { nameWithOwner: string };
            };
          }[];
        };
      };
    };
  };
}

async function resolveLinkedPRs(cfg: ParityConfig, issueNumber: number): Promise<LinkedPR[]> {
  try {
    const root = await ghGraphQL<TimelineRoot>(TIMELINE_QUERY, {
      owner: cfg.github.owner,
      repo: cfg.github.repo,
      num: issueNumber,
    });
    const nodes = root.data?.repository?.issue?.timelineItems?.nodes ?? [];
    const out: LinkedPR[] = [];
    for (const n of nodes) {
      const s = n.source;
      if (!s || s.number === undefined) continue;
      const state: LinkedPR["state"] = s.merged ? "merged" : s.state?.toLowerCase() === "open" ? "open" : "closed";
      out.push({
        repo: s.repository?.nameWithOwner ?? `${cfg.github.owner}/${cfg.github.repo}`,
        number: s.number,
        state,
        mergedAt: s.mergedAt ?? undefined,
        author: s.author?.login,
        url: s.url,
      });
    }
    return out;
  } catch {
    return [];
  }
}

function parseConnectorAndFlow(title: string): { connector: string; flow: string } {
  // Conventions observed:
  //   "[parity] stripe / capture / amount.currency"
  //   "parity(stripe/capture): metadata.user_id mismatch"
  //   "stripe: capture flow returns wrong currency"
  const titleLower = title.toLowerCase();
  const parens = titleLower.match(/parity\(([^/]+)\/([^)]+)\)/);
  if (parens) return { connector: parens[1].trim(), flow: parens[2].trim() };

  const bracket = titleLower.match(/\[parity\]\s*([^/\s]+)\s*\/\s*([^/\s]+)/);
  if (bracket) return { connector: bracket[1].trim(), flow: bracket[2].trim() };

  const colon = titleLower.match(/^([a-z0-9_-]+)\s*:\s*(authorize|capture|refund|void|psync|rsync|webhook|setup_mandate|repeat_payment|dispute)/);
  if (colon) return { connector: colon[1], flow: colon[2] };

  return { connector: "unknown", flow: "unknown" };
}

async function flattenSubtree(
  cfg: ParityConfig,
  rootNum: number,
  parentTracking: number,
  acc: Leaf[],
): Promise<void> {
  const node = await fetchNode(cfg, rootNum);
  if (!node) return;

  const sub = node.subIssues?.nodes ?? [];
  if (sub.length > 0) {
    for (const child of sub) {
      // child is fetched again so we have body + labels (the sub-issue list is shallow)
      await flattenSubtree(cfg, child.number, rootNum, acc);
    }
    return;
  }

  // Try task-list fallback: maybe body lists child issues even though subIssues is empty
  const taskRefs = node.body ? parseTaskListIssueRefs(node.body, cfg.github.owner, cfg.github.repo) : [];
  const sameRepoRefs = taskRefs.filter(
    (r) => r.repo === `${cfg.github.owner}/${cfg.github.repo}` && r.number !== node.number,
  );
  if (sameRepoRefs.length > 0) {
    for (const ref of sameRepoRefs) {
      await flattenSubtree(cfg, ref.number, rootNum, acc);
    }
    return;
  }

  // It's a leaf.
  const labels = node.labels?.nodes.map((l) => l.name) ?? [];
  const linkedPRs = await resolveLinkedPRs(cfg, node.number);
  const { connector, flow } = parseConnectorAndFlow(node.title);
  acc.push({
    number: node.number,
    title: node.title,
    body: node.body ?? "",
    labels,
    createdAt: node.createdAt ?? new Date().toISOString(),
    url: node.url,
    linkedPRs,
    parentTracking,
    connector,
    flow,
  });
}

export async function walkTree(cfg: ParityConfig): Promise<Leaf[]> {
  const cachePath = join(cfg.cache.dir, dailyTreeCachePath(cfg.cache.dir).split("/").pop()!);
  const fresh = await readIfFresh<Leaf[]>(cachePath, cfg.cache.treeTtlMs);
  if (fresh) return fresh;

  const leaves: Leaf[] = [];
  await flattenSubtree(cfg, cfg.github.rootIssue, cfg.github.rootIssue, leaves);
  await writeAtomic(cachePath, leaves);
  return leaves;
}

// Helpers exposed for tests & callers that need raw access:
export { parseConnectorAndFlow, parseTaskListIssueRefs, fetchNode, resolveLinkedPRs };

// Refresh a single leaf's comments — used by phases that need the latest Understanding/Plan comment.
export async function refetchLeafCommentsLatest(cfg: ParityConfig, num: number) {
  return getIssue(`${cfg.github.owner}/${cfg.github.repo}`, num);
}
