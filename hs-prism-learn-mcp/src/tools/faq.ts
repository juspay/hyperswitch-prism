import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { faqShape } from "../schemas.js";
import { getDoc, getDocSection } from "../data/knowledge.js";
import { result } from "../util.js";
import { cite, renderSources } from "../citations.js";
import { ANNOTATIONS } from "./_shared.js";

const FAQ_PATH = "docs/FAQs.md";

export function registerFaq(server: McpServer): void {
  server.registerTool(
    "prism_learn_faq",
    {
      title: "Answer from the project FAQ",
      description:
        "Match a question against docs/FAQs.md and return the verbatim answer, or list all FAQ questions. Good for 'how is Prism different?', 'how does it handle PCI?', 'how many processors are supported?'.",
      inputSchema: faqShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      const doc = getDoc(FAQ_PATH);
      const source = cite(FAQ_PATH, "the project FAQ");
      const questions = (doc?.headings ?? []).filter((h) => h.level === 3).map((h) => h.text);

      if (!args.query) {
        const text =
          `# FAQ — ${questions.length} questions\n\n` +
          questions.map((q) => `- ${q}`).join("\n") +
          `\n\nAsk one with \`prism_learn_faq { query: "..." }\`.` +
          renderSources([source]);
        return result(text, { ok: true, matches: [], questions, sourcePath: FAQ_PATH });
      }

      // Score questions by token overlap, return the best section verbatim.
      const tokens = args.query.toLowerCase().split(/[^a-z0-9]+/).filter((t) => t.length > 2);
      const ranked = questions
        .map((q) => {
          const ql = q.toLowerCase();
          let score = 0;
          for (const t of tokens) if (ql.includes(t)) score += 1;
          return { q, score };
        })
        .filter((x) => x.score > 0)
        .sort((a, b) => b.score - a.score);

      if (!ranked.length) {
        return result(
          `No FAQ question matched "${args.query}". All questions:\n` + questions.map((q) => `- ${q}`).join("\n") + renderSources([source]),
          { ok: true, query: args.query, matches: [], questions, sourcePath: FAQ_PATH },
        );
      }

      const matches = ranked.slice(0, 3).map((r) => {
        const section = getDocSection(FAQ_PATH, r.q);
        return { question: r.q, answer: section?.text ?? "" };
      });
      const text =
        `# FAQ: "${args.query}"\n\n` +
        matches.map((m) => m.answer || `### ${m.question}`).join("\n\n---\n\n") +
        renderSources([source]);
      return result(text, { ok: true, query: args.query, matches, sourcePath: FAQ_PATH });
    },
  );
}
