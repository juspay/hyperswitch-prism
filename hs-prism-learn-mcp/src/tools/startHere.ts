import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { startHereShape } from "../schemas.js";
import { getPathForRole, listPaths, getConcept, findSkillForTask } from "../data/knowledge.js";
import { result } from "../util.js";
import { cite, renderSources, type Citation } from "../citations.js";
import { ANNOTATIONS } from "./_shared.js";

const NEXT_TOOLS =
  "- `prism_learn_explain_concept` — what is X? (try depth=tldr)\n" +
  "- `prism_learn_architecture_overview` — the big picture, end to end\n" +
  "- `prism_learn_repo_map` / `prism_learn_search` — where does X live?\n" +
  "- `prism_learn_how_to` — how do I do X? (routes to the right skill)\n" +
  "- `prism_learn_learning_path` — a step-by-step curriculum for your role\n" +
  "- `prism_learn_read_doc` — read any repo doc verbatim";

export function registerStartHere(server: McpServer): void {
  server.registerTool(
    "prism_learn_start_here",
    {
      title: "Start here: orient a newcomer to hyperswitch-prism",
      description:
        "The entry point for anyone new to this repo. Explains what the project is in one breath, picks a learning path for your role or goal, and points to the next tools. Call this first.",
      inputSchema: startHereShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      const citations: Citation[] = [
        cite("README.md", "the project overview"),
        cite("docs/SUMMARY.md", "the documentation table of contents"),
      ];

      // Pick a path: by explicit role, else infer from goal, else the default explorer tour.
      let path = args.role ? getPathForRole(args.role) : undefined;
      let inferredFrom = args.role ? `role=${args.role}` : "";
      if (!path && args.goal) {
        const skill = findSkillForTask(args.goal);
        // crude goal->path mapping
        const g = args.goal.toLowerCase();
        if (/\bconnector\b/.test(g) && /\b(new|add|build|create)\b/.test(g)) path = getPathForRole("contributor");
        else if (/\b(flow|payment method|wallet|refund|capture)\b/.test(g)) path = listPaths().find((p) => p.id === "add-flow-or-payment-method");
        else if (/\b(review|pr)\b/.test(g)) path = getPathForRole("reviewer");
        else if (/\b(sdk|app|integrate)\b/.test(g)) path = getPathForRole("integrator");
        else path = getPathForRole("explorer");
        inferredFrom = `goal "${args.goal}"` + (skill ? ` (skill: ${skill.skill.name})` : "");
      }
      if (!path) path = getPathForRole("explorer");

      const whatIsPrism = getConcept("what-is-prism");
      const intro =
        "# Welcome to hyperswitch-prism\n\n" +
        (whatIsPrism ? `${whatIsPrism.depth.tldr}\n\n` : "") +
        "**This `prism_learn_*` server is your guide to the repo.** Every answer it gives is grounded in real files in this repo — it never makes facts up. If it doesn't know, it says so.\n";

      const pathBlock = path
        ? `## Suggested path: ${path.title}\n${path.summary}\n\n` +
          path.steps.map((s, i) => `${i + 1}. **${s.slug}** — ${s.why}${s.file ? ` (\`${s.file}\`)` : ""}`).join("\n") +
          `\n\n_Get the full curriculum with_ \`prism_learn_learning_path { role: "${path.role}" }\`.`
        : "";

      const text =
        intro +
        `\n${pathBlock}\n\n## What you can ask next\n${NEXT_TOOLS}` +
        renderSources(citations);

      return result(text, {
        ok: true,
        persona: path?.role ?? "explorer",
        inferredFrom,
        learningPath: path ? { id: path.id, title: path.title, steps: path.steps } : null,
        availableRoles: listPaths().map((p) => p.role),
        nextTools: [
          "prism_learn_explain_concept",
          "prism_learn_architecture_overview",
          "prism_learn_repo_map",
          "prism_learn_search",
          "prism_learn_how_to",
          "prism_learn_learning_path",
        ],
        citations,
      });
    },
  );
}
