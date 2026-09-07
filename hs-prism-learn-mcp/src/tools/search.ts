import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { searchShape } from "../schemas.js";
import { searchDocs } from "../search/index.js";
import { result } from "../util.js";
import { ANNOTATIONS } from "./_shared.js";

export function registerSearch(server: McpServer): void {
  server.registerTool(
    "prism_learn_search",
    {
      title: "Search the repo's docs, skills, and grace guides",
      description:
        "Full-text keyword search across all indexed markdown (architecture docs, getting-started, the .skills playbooks, and grace guides). Returns ranked real files with a snippet and a link. Use prism_learn_read_doc to read a hit verbatim.",
      inputSchema: searchShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      const area = args.area ?? "all";
      const limit = args.limit ?? 8;
      const hits = searchDocs(args.query, area, limit);
      if (!hits.length) {
        return result(
          `No docs matched "${args.query}" in area "${area}". Try fewer or different keywords, or prism_learn_repo_map to locate a file by topic.`,
          { ok: true, query: args.query, area, results: [] },
        );
      }
      const text =
        `# Search: "${args.query}" (${hits.length} hits in ${area})\n\n` +
        hits
          .map((h) => `- **${h.title}** — \`${h.path}\` _(${h.category})_\n  ${h.snippet}`)
          .join("\n");
      return result(text, {
        ok: true,
        query: args.query,
        area,
        results: hits,
      });
    },
  );
}
