import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { repoMapShape } from "../schemas.js";
import { searchRepoMap, listRepoAreas, topDirs } from "../data/knowledge.js";
import { result } from "../util.js";
import { cite, type Citation } from "../citations.js";
import { ANNOTATIONS } from "./_shared.js";

export function registerRepoMap(server: McpServer): void {
  server.registerTool(
    "prism_learn_repo_map",
    {
      title: "Find where something lives in the repo",
      description:
        "Answer 'where is X?' — map a topic or symbol (e.g. 'stripe', 'proto', 'macros', 'flow dependencies', 'glossary') to the exact, verified repo path. Omit the query for the full top-level map.",
      inputSchema: repoMapShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      if (!args.query) {
        const areas = listRepoAreas();
        const text =
          "# Repo map\n\n## Top-level folders\n" +
          topDirs().map((d) => `- \`${d}/\``).join("\n") +
          "\n\n## Key locations\n" +
          areas.map((a) => `- **${a.topic}** — \`${a.path}\`\n  ${a.what}`).join("\n");
        return result(text, { ok: true, areas, topDirs: topDirs() });
      }

      const matches = searchRepoMap(args.query);
      if (!matches.length) {
        return result(
          `No mapped location matches "${args.query}". Try prism_learn_search for a full-text search across docs, or browse the top-level map by calling this tool with no query.`,
          { ok: true, query: args.query, matches: [] },
        );
      }
      const citations: Citation[] = matches.slice(0, 8).map((a) => cite(a.path, a.topic));
      const text =
        `# Where "${args.query}" lives\n\n` +
        matches.map((a) => `- **${a.topic}** — \`${a.path}\`\n  ${a.what}`).join("\n");
      return result(text, { ok: true, query: args.query, matches, citations });
    },
  );
}
