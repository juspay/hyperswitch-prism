import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { explainConceptShape } from "../schemas.js";
import { getConcept, suggestConcepts } from "../data/knowledge.js";
import { result, errorResult } from "../util.js";
import { renderSources } from "../citations.js";
import { ANNOTATIONS, renderConcept, conceptCitations } from "./_shared.js";

export function registerExplainConcept(server: McpServer): void {
  server.registerTool(
    "prism_learn_explain_concept",
    {
      title: "Explain a hyperswitch-prism concept in plain language",
      description:
        "Answer 'what is X?' for a core concept (connector, transformer, flow, RouterDataV2, macros, grace, proto, status codes, ...). " +
        "Returns a plain-language explanation with an analogy and links to the real files that back it. Use depth=tldr for non-engineers.",
      inputSchema: explainConceptShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      const card = getConcept(args.concept);
      if (!card) {
        const suggestions = suggestConcepts(args.concept);
        return errorResult(
          `No concept card for "${args.concept}". I only answer from grounded cards, so I won't guess.` +
            (suggestions.length ? `\n\nDid you mean: ${suggestions.join(", ")}?` : "") +
            `\n\nTry prism_learn_search to find docs, or prism_learn_glossary for a term lookup.`,
          { concept: args.concept, suggestions },
        );
      }
      const citations = conceptCitations(card);
      const text = renderConcept(card, args.depth) + renderSources(citations);
      return result(text, {
        ok: true,
        slug: card.slug,
        title: card.title,
        depth: args.depth ?? "standard",
        analogy: card.analogy,
        prerequisites: card.prerequisites,
        related: card.related,
        citations,
      });
    },
  );
}
