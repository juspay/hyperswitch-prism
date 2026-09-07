import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { coverageShape } from "../schemas.js";
import {
  getConnector,
  suggestConnectors,
  flowsForConnector,
  connectorsForFlow,
  getCoverage,
  META,
} from "../data/knowledge.js";
import { result } from "../util.js";
import { cite, renderSources } from "../citations.js";
import { ANNOTATIONS } from "./_shared.js";

export function registerCoverage(server: McpServer): void {
  server.registerTool(
    "prism_learn_coverage",
    {
      title: "Which connectors support which flows",
      description:
        "Answer coverage questions from the generated matrix: which connectors support a flow, what flows a connector supports, or whether a specific connector supports a specific flow. Best-effort parse of docs-generated/all_connector.md.",
      inputSchema: coverageShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      const cov = getCoverage();
      const source = cite(cov.sourcePath, "the full connector coverage matrix");

      // connector + flow → does X support Y?
      if (args.connector && args.flow) {
        const c = getConnector(args.connector);
        const flows = flowsForConnector(args.connector);
        const supports = flows.some((f) => f.toLowerCase().includes(args.flow!.toLowerCase()));
        const text =
          `**${args.connector}** ${supports ? "supports" : "does NOT appear to support"} a flow matching "${args.flow}".\n\n` +
          (flows.length ? `Flows it supports: ${flows.join(", ")}` : "No coverage rows found for it.") +
          renderSources([source]);
        return result(text, {
          ok: true,
          connector: args.connector,
          flow: args.flow,
          supports,
          connectorFlows: flows,
          implPath: c?.implPath ?? null,
          sourcePath: cov.sourcePath,
        });
      }

      // connector only → what does it support?
      if (args.connector) {
        const c = getConnector(args.connector);
        const flows = flowsForConnector(args.connector);
        if (!c && !flows.length) {
          return result(
            `No connector "${args.connector}" found.` +
              (suggestConnectors(args.connector).length ? ` Did you mean: ${suggestConnectors(args.connector).join(", ")}?` : ""),
            { ok: true, connector: args.connector, supportedFlows: [], suggestions: suggestConnectors(args.connector) },
          );
        }
        const text =
          `# ${args.connector} — supported flows (${flows.length})\n\n` +
          (flows.length ? flows.map((f) => `- ${f}`).join("\n") : "_No coverage rows found._") +
          (c ? `\n\nSource code: \`${c.implPath}\`` : "") +
          renderSources([source]);
        return result(text, {
          ok: true,
          connector: args.connector,
          supportedFlows: flows,
          implPath: c?.implPath ?? null,
          sourcePath: cov.sourcePath,
        });
      }

      // flow only → which connectors support it?
      if (args.flow) {
        const groups = connectorsForFlow(args.flow);
        if (!groups.length) {
          return result(`No coverage data matched a flow like "${args.flow}". Known flow labels: ${Object.keys(cov.flows).slice(0, 12).join(", ")}...`, {
            ok: true,
            flow: args.flow,
            matches: [],
            knownFlows: Object.keys(cov.flows),
          });
        }
        const text =
          `# Connectors supporting "${args.flow}"\n\n` +
          groups
            .map((g) => `## ${g.flow} (${g.connectors.length})\n${g.connectors.join(", ")}`)
            .join("\n\n") +
          renderSources([source]);
        return result(text, { ok: true, flow: args.flow, matches: groups, sourcePath: cov.sourcePath });
      }

      // summary
      const text =
        `# Coverage summary\n\n` +
        `- Connectors in the on-disk registry: **${META.connectorCounts.registry}** (the generated llms.txt digest says ${META.connectorCounts.llmsTxt}).\n` +
        `- Flows tracked in the matrix: **${Object.keys(cov.flows).length}**.\n\n` +
        `Ask with \`flow\` (which connectors support it) or \`connector\` (what it supports). ${cov.note}` +
        renderSources([source]);
      return result(text, {
        ok: true,
        connectorCounts: META.connectorCounts,
        flows: Object.keys(cov.flows),
        sourcePath: cov.sourcePath,
      });
    },
  );
}
