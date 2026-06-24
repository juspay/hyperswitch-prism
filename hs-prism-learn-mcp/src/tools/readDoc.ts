import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { readDocShape } from "../schemas.js";
import { getDoc, getBody, getDocSection, suggestDocPaths } from "../data/knowledge.js";
import { result, errorResult } from "../util.js";
import { ANNOTATIONS } from "./_shared.js";

export function registerReadDoc(server: McpServer): void {
  server.registerTool(
    "prism_learn_read_doc",
    {
      title: "Read a repo doc verbatim",
      description:
        "Return the exact content of an indexed repo doc (or one of its sections), so answers quote the source instead of paraphrasing it. Pair with prism_learn_search to find the path first.",
      inputSchema: readDocShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      const doc = getDoc(args.path);
      if (!doc) {
        const suggestions = suggestDocPaths(args.path);
        return errorResult(
          `No indexed doc at "${args.path}". This server only reads files it has indexed.` +
            (suggestions.length ? `\n\nClosest indexed paths:\n${suggestions.map((p) => `- \`${p}\``).join("\n")}` : "") +
            `\n\nUse prism_learn_search to find a doc by keywords.`,
          { path: args.path, suggestions },
        );
      }

      if (args.heading) {
        const section = getDocSection(doc.path, args.heading);
        if (!section) {
          return errorResult(
            `Doc "${doc.path}" has no heading matching "${args.heading}". Available headings:\n` +
              doc.headings.map((h) => `- ${"  ".repeat(h.level - 1)}${h.text}`).join("\n"),
            { path: doc.path, headings: doc.headings },
          );
        }
        return result(`> Source: \`${doc.path}\` (${doc.githubUrl})\n\n${section.text}`, {
          ok: true,
          path: doc.path,
          title: doc.title,
          heading: section.heading,
          body: section.text,
          githubUrl: doc.githubUrl,
        });
      }

      const body = getBody(doc.path) ?? "";
      return result(`> Source: \`${doc.path}\` (${doc.githubUrl})\n\n${body}`, {
        ok: true,
        path: doc.path,
        title: doc.title,
        headings: doc.headings,
        body,
        githubUrl: doc.githubUrl,
      });
    },
  );
}
