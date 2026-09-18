import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { architectureOverviewShape } from "../schemas.js";
import { getConcept } from "../data/knowledge.js";
import { result } from "../util.js";
import { cite, renderSources, type Citation } from "../citations.js";
import { ANNOTATIONS, depthText, renderConcept } from "./_shared.js";

// The ordered story of how Prism works, told through real concept cards.
const TOUR = [
  "what-is-prism",
  "three-layer-architecture",
  "unified-request",
  "connector",
  "transformer",
  "flow",
  "payment-proto",
  "status-codes",
  "grace",
];

export function registerArchitectureOverview(server: McpServer): void {
  server.registerTool(
    "prism_learn_architecture_overview",
    {
      title: "A plain-language tour of how hyperswitch-prism works",
      description:
        "Give a newcomer the big picture end to end: the unified request, the three layers, connectors and transformers, flows, the proto contract, status codes, and grace codegen. " +
        "depth=tldr is non-technical; depth=deep adds engineer detail. Pass a topic to zoom into one concept.",
      inputSchema: architectureOverviewShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      const depth = args.depth ?? "standard";

      if (args.topic) {
        const card = getConcept(args.topic);
        if (card) {
          const citations: Citation[] = card.go_deeper.map((g) => cite(g.path, g.why));
          return result(renderConcept(card, depth) + renderSources(citations), {
            ok: true,
            topic: card.slug,
            depth,
            citations,
          });
        }
      }

      const cards = TOUR.map((s) => getConcept(s)).filter((c): c is NonNullable<typeof c> => Boolean(c));
      const sections = cards.map((c) => `## ${c.title}\n${depthText(c, depth)}`);
      const citations: Citation[] = [cite("docs/architecture/README.md", "the architecture overview with diagrams")];
      for (const c of cards) {
        const g = c.go_deeper[0];
        if (g && !citations.some((x) => x.path === g.path)) citations.push(cite(g.path, g.why));
      }

      const intro =
        "# How hyperswitch-prism works\n\n" +
        "You send ONE unified payment request. Prism translates it into whatever each payment processor expects, " +
        "sends it, and translates the reply back. Here is the whole picture, layer by layer:\n";

      const text = `${intro}\n${sections.join("\n\n")}` + renderSources(citations);
      return result(text, {
        ok: true,
        depth,
        components: cards.map((c) => ({ slug: c.slug, title: c.title })),
        dataFlow: ["unified request", "connector + transformer", "processor API", "unified response", "numeric status"],
        citations,
      });
    },
  );
}
