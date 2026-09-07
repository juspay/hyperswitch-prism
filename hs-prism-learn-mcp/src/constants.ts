/** Shared constants: package identity, reference URLs, GitHub blob base, resource URIs. */

export const SERVER_NAME = "hs-prism-learn";
export const SERVER_VERSION = "0.1.0";

/** Canonical repo + default branch, used to build clickable GitHub links from repo-relative paths. */
export const REPO_URL = "https://github.com/juspay/hyperswitch-prism";
export const REPO_BRANCH = "main";
export const GITHUB_BLOB_BASE = `${REPO_URL}/blob/${REPO_BRANCH}`;

/** Official LLM-oriented digest the repo maintains; surfaced as-is by a resource. */
export const LLMS_TXT_URL =
  "https://raw.githubusercontent.com/juspay/hyperswitch-prism/main/docs-generated/llms.txt";

/** Personas a newcomer can self-identify as (drives start_here + learning_path). */
export const ROLES = ["explorer", "contributor", "reviewer", "integrator"] as const;
export type Role = (typeof ROLES)[number];

/** How much detail an explanation should carry. */
export const DEPTHS = ["tldr", "standard", "deep"] as const;
export type Depth = (typeof DEPTHS)[number];

/** Search corpora the doc search can scope to. */
export const SEARCH_AREAS = ["docs", "skills", "grace", "all"] as const;
export type SearchArea = (typeof SEARCH_AREAS)[number];

/** Resource URIs. */
export const RESOURCE_URIS = {
  start: "prism://learn/start",
  architecture: "prism://learn/architecture",
  glossary: "prism://learn/glossary",
  repoMap: "prism://learn/repo-map",
  skillsIndex: "prism://learn/skills-index",
  payments101: "prism://learn/payments-101",
  llmsTxt: "prism://learn/llms-txt",
} as const;
