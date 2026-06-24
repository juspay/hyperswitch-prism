/** Turn repo-relative paths into clickable GitHub links for tool citations. */
import { GITHUB_BLOB_BASE } from "./constants.js";

export interface Citation {
  path: string; // repo-relative, e.g. "docs/architecture/README.md"
  githubUrl: string;
  why?: string;
}

/** Normalize a repo-relative path: strip leading "./" and "/". */
export function normalizeRepoPath(path: string): string {
  return path.replace(/^\.\//, "").replace(/^\/+/, "").trim();
}

/** Build the canonical GitHub blob URL for a repo-relative path (+ optional #anchor). */
export function githubUrl(path: string, anchor?: string): string {
  const clean = normalizeRepoPath(path);
  const hash = anchor ? `#${anchor}` : "";
  return `${GITHUB_BLOB_BASE}/${clean}${hash}`;
}

/** Build a citation object from a repo-relative path. */
export function cite(path: string, why?: string): Citation {
  const clean = normalizeRepoPath(path);
  return why ? { path: clean, githubUrl: githubUrl(clean), why } : { path: clean, githubUrl: githubUrl(clean) };
}

/** Render a list of citations as a markdown "Sources" block. */
export function renderSources(citations: Citation[]): string {
  if (!citations.length) return "";
  const lines = citations.map((c) => `- \`${c.path}\`${c.why ? ` — ${c.why}` : ""}`);
  return `\n\n**Sources** (real files in this repo):\n${lines.join("\n")}`;
}
