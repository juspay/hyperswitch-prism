import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { glossaryShape } from "../schemas.js";
import { getGlossaryTerm, searchGlossary, listGlossary } from "../data/knowledge.js";
import { result, errorResult } from "../util.js";
import { cite, renderSources } from "../citations.js";
import { ANNOTATIONS } from "./_shared.js";

export function registerGlossary(server: McpServer): void {
  server.registerTool(
    "prism_learn_glossary",
    {
      title: "Look up a hyperswitch-prism / payments term",
      description:
        "Define a single term (e.g. 'mandate', 'BNPL', 'FFI', 'DSL') from the repo's generated glossary, or list every term. Definitions are taken verbatim from docs-generated/glossary.md.",
      inputSchema: glossaryShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      const source = cite("docs-generated/glossary.md", "the repo's A-Z glossary");

      if (!args.term) {
        const terms = listGlossary();
        const text =
          `# Glossary (${terms.length} terms)\n\n` +
          terms.map((t) => `- **${t.term}** — ${t.definition}`).join("\n") +
          renderSources([source]);
        return result(text, { ok: true, terms, citations: [source] });
      }

      const exact = getGlossaryTerm(args.term);
      if (exact) {
        const text = `**${exact.term}** — ${exact.definition}` + renderSources([source]);
        return result(text, { ok: true, terms: [exact], citations: [source] });
      }

      const matches = searchGlossary(args.term);
      if (matches.length) {
        const text =
          `No exact term "${args.term}". Closest matches:\n\n` +
          matches.map((t) => `- **${t.term}** — ${t.definition}`).join("\n") +
          renderSources([source]);
        return result(text, { ok: true, terms: matches, citations: [source] });
      }

      return errorResult(`No glossary term matches "${args.term}". Try prism_learn_search or prism_learn_explain_concept.`, {
        term: args.term,
        terms: [],
      });
    },
  );
}
