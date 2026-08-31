import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { connectorWalkthroughShape } from "../schemas.js";
import { getConnector, suggestConnectors, flowsForConnector, getConcept } from "../data/knowledge.js";
import { result, errorResult } from "../util.js";
import { cite, renderSources, type Citation } from "../citations.js";
import { ANNOTATIONS } from "./_shared.js";

export function registerConnectorWalkthrough(server: McpServer): void {
  server.registerTool(
    "prism_learn_connector_walkthrough",
    {
      title: "Walk through a real connector",
      description:
        "Tour one real connector (default 'stripe') so a newcomer can see the connector shape in practice: the main file, the transformers file, and which flows it supports. Every connector follows this same shape.",
      inputSchema: connectorWalkthroughShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      const name = args.connector ?? "stripe";
      const c = getConnector(name);
      if (!c) {
        const suggestions = suggestConnectors(name);
        return errorResult(
          `No connector named "${name}" in the registry.` +
            (suggestions.length ? `\n\nDid you mean: ${suggestions.join(", ")}?` : "") +
            `\n\nUse prism_learn_repo_map to find the connectors folder.`,
          { connector: name, suggestions },
        );
      }
      const supportedFlows = flowsForConnector(c.name);
      const card = getConcept("connector");
      const citations: Citation[] = [
        cite(c.implPath, "the connector's main file: struct + macro-based flow wiring"),
      ];
      if (c.transformersPath) citations.push(cite(c.transformersPath, "the transformers: request/response mapping"));
      citations.push(cite("crates/integrations/connector-integration/src/connectors.rs", "the registry that lists this connector"));

      const lines: string[] = [];
      lines.push(`# Walkthrough: the \`${c.name}\` connector`);
      lines.push("");
      if (card) lines.push(`${card.depth.standard ?? card.depth.tldr}\n`);
      lines.push(`## The two files`);
      lines.push(`1. **\`${c.implPath}\`** — declares the connector struct and uses the macros (\`create_all_prerequisites!\` and \`macro_connector_implementation!\`) to wire up each flow.`);
      lines.push(
        c.transformersPath
          ? `2. **\`${c.transformersPath}\`** — the transformers: \`TryFrom\` implementations that map the unified request to ${c.name}'s API and the response back, matching on \`PaymentMethodData\`.`
          : `2. _(no transformers.rs found for this connector)_`,
      );
      if (supportedFlows.length) {
        lines.push("");
        lines.push(`## Flows it supports (from the coverage matrix)`);
        lines.push(supportedFlows.map((f) => `- ${f}`).join("\n"));
      }
      lines.push("");
      lines.push(`_Tip: open \`${c.implPath}\` and \`${c.transformersPath ?? "its transformers"}\` side by side — once you have read these, every other connector reads the same way._`);

      return result(lines.join("\n") + renderSources(citations), {
        ok: true,
        connector: c.name,
        implPath: c.implPath,
        transformersPath: c.transformersPath,
        supportedFlows,
        citations,
      });
    },
  );
}
