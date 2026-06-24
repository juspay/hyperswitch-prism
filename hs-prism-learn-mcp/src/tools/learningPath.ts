import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { learningPathShape } from "../schemas.js";
import { getPathForRole, getConcept } from "../data/knowledge.js";
import { result, errorResult } from "../util.js";
import { cite, renderSources, type Citation } from "../citations.js";
import { ANNOTATIONS } from "./_shared.js";

export function registerLearningPath(server: McpServer): void {
  server.registerTool(
    "prism_learn_learning_path",
    {
      title: "Get a step-by-step curriculum for your role",
      description:
        "Return an ordered learning path for a role: explorer (understand a payment end to end), contributor (add/fix a connector), reviewer (review a PR), or integrator (use the SDK). Each step links a concept and a real file.",
      inputSchema: learningPathShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      const path = getPathForRole(args.role);
      if (!path) {
        return errorResult(`No learning path for role "${args.role}".`, { role: args.role });
      }
      const citations: Citation[] = [];
      const stepLines = path.steps.map((s, i) => {
        const card = getConcept(s.slug);
        const title = card ? card.title : s.slug;
        if (s.file && !citations.some((c) => c.path === s.file)) citations.push(cite(s.file, s.why));
        return `${i + 1}. **${title}** — ${s.why}${s.file ? `\n   → \`${s.file}\`` : ""}\n   _explain with_ \`prism_learn_explain_concept { concept: "${s.slug}" }\``;
      });
      const text =
        `# ${path.title}\n\n${path.summary}\n\n${stepLines.join("\n\n")}` + renderSources(citations);
      return result(text, {
        ok: true,
        role: path.role,
        path: { id: path.id, title: path.title, summary: path.summary, steps: path.steps },
        citations,
      });
    },
  );
}
