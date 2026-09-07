/** Lightweight keyword ranker over the indexed docs. No deps; deterministic. */
import { listDocs, type DocEntry } from "../data/knowledge.js";
import type { SearchArea } from "../constants.js";

export interface SearchHit {
  path: string;
  title: string;
  category: string;
  snippet: string;
  score: number;
  githubUrl: string;
}

function inArea(d: DocEntry, area: SearchArea): boolean {
  switch (area) {
    case "skills":
      return d.category.startsWith("skill") || d.category === "flow-pattern";
    case "grace":
      return d.category === "grace";
    case "docs":
      return (
        d.category.startsWith("architecture") ||
        d.category === "getting-started" ||
        d.category === "payment-method" ||
        d.category === "docs" ||
        d.category === "blog" ||
        d.category === "generated" ||
        d.category === "overview"
      );
    case "all":
    default:
      return true;
  }
}

export function searchDocs(query: string, area: SearchArea = "all", limit = 8): SearchHit[] {
  const tokens = query.toLowerCase().split(/[^a-z0-9]+/).filter((t) => t.length > 1);
  if (!tokens.length) return [];
  const hits: SearchHit[] = [];

  for (const d of listDocs()) {
    if (!inArea(d, area)) continue;
    const path = d.path.toLowerCase();
    const title = d.title.toLowerCase();
    const summary = d.summary.toLowerCase();
    const headingText = d.headings.map((h) => h.text.toLowerCase()).join(" ");
    const keywords = d.keywords.join(" ");

    let score = 0;
    for (const t of tokens) {
      if (path.includes(t)) score += 3;
      if (title.includes(t)) score += 3;
      if (keywords.includes(t)) score += 2;
      if (headingText.includes(t)) score += 1;
      if (summary.includes(t)) score += 1;
    }
    // small boost when the whole phrase appears in the title
    if (title.includes(query.toLowerCase())) score += 2;

    if (score > 0) {
      hits.push({
        path: d.path,
        title: d.title,
        category: d.category,
        snippet: d.summary || d.headings.slice(0, 3).map((h) => h.text).join(" · "),
        score,
        githubUrl: d.githubUrl,
      });
    }
  }

  return hits.sort((a, b) => b.score - a.score || a.path.localeCompare(b.path)).slice(0, limit);
}
