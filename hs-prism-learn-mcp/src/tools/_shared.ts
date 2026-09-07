/** Helpers shared across the learn tools. */
import { cite, type Citation } from "../citations.js";
import type { ConceptCard } from "../data/knowledge.js";
import type { Depth } from "../constants.js";

const READONLY = { readOnlyHint: true, openWorldHint: false } as const;
export const ANNOTATIONS = READONLY;

/** Pick the prose for a requested depth, falling back gracefully. */
export function depthText(card: ConceptCard, depth?: Depth): string {
  const d = depth ?? "standard";
  if (d === "tldr") return card.depth.tldr;
  if (d === "deep") return card.depth.deep || card.body || card.depth.standard || card.depth.tldr;
  return card.depth.standard || card.depth.tldr;
}

/** Real-file citations backing a concept card (go_deeper + any anchor files). */
export function conceptCitations(card: ConceptCard): Citation[] {
  const out = card.go_deeper.map((g) => cite(g.path, g.why));
  for (const a of card.verify_anchors) {
    if (!out.some((c) => c.path === a.path)) out.push(cite(a.path, "referenced by this concept"));
  }
  return out;
}

/** Render a concept card to markdown at the requested depth. */
export function renderConcept(card: ConceptCard, depth?: Depth): string {
  const lines: string[] = [];
  lines.push(`# ${card.title}`);
  lines.push("");
  lines.push(`**In one line:** ${card.one_liner}`);
  if (card.analogy) {
    lines.push("");
    lines.push(`**Analogy:** ${card.analogy}`);
  }
  lines.push("");
  lines.push(depthText(card, depth));
  if (card.prerequisites.length) {
    lines.push("");
    lines.push(`**First understand:** ${card.prerequisites.join(", ")}`);
  }
  if (card.related.length) {
    lines.push("");
    lines.push(`**Related concepts:** ${card.related.join(", ")}`);
  }
  return lines.join("\n");
}
