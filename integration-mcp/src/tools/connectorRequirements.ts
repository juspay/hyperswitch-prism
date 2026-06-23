import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { connectorRequirementsShape } from "../schemas.js";
import {
  getConnector,
  suggestConnectors,
  requiredFields,
  optionalFields,
  envVarName,
} from "../data/connectors.js";
import { buildConfigObject } from "../config.js";
import { result, errorResult } from "../util.js";

export function registerConnectorRequirements(server: McpServer): void {
  server.registerTool(
    "prism_connector_requirements",
    {
      title: "Prism connector credential requirements",
      description:
        "Resolve the EXACT credential fields a connector needs, from payment.proto (no guessing). " +
        "Input: { connector: '<machine name>' } e.g. 'stripe', 'authorizedotnet', 'adyen'. " +
        "Returns: required & optional field names (camelCase, as used in connectorConfig), which are secrets " +
        "(wrapped as { value }), a ready-to-fill connectorConfig template, suggested env var names, sandbox test " +
        "card(s), base-URL override support, and docs link. This removes the #1 source of integration pain.",
      inputSchema: connectorRequirementsShape,
      annotations: { readOnlyHint: true, openWorldHint: false },
    },
    (args) => {
      const c = getConnector(args.connector);
      if (!c) {
        const suggestions = suggestConnectors(args.connector);
        return errorResult(
          `Unknown connector '${args.connector}'.` +
            (suggestions.length ? ` Did you mean: ${suggestions.join(", ")}?` : "") +
            ` Run prism_list_connectors to see all options.`,
          { connector: args.connector, suggestions },
        );
      }

      const req = requiredFields(c);
      const opt = optionalFields(c);
      const template = { connectorConfig: { [c.connector]: buildConfigObject(c, undefined) } };

      const fieldLines = req.map((f) => {
        const doc = c.fieldDocs[f.name]?.description;
        const shape = f.secret ? `{ value: "..." }` : f.type === "string[]" ? `[ ... ]` : `"..."`;
        return `  - **${f.name}** ${f.secret ? "(secret)" : ""} → ${shape}` + (doc ? ` — ${doc}` : "");
      });
      const optLines = opt.map(
        (f) => `  - ${f.name}${f.secret ? " (secret)" : ""}${f.isBaseUrl ? "" : ""}`,
      );
      const envLines = req.map((f) => `${envVarName(c.connector, f)}=${c.fieldDocs[f.name]?.example ?? ""}`);
      const cards = c.sandboxCards
        .map((card) => `  - ${card.brand}: ${card.number} exp ${card.expMonth}/${card.expYear} cvc ${card.cvc}`)
        .join("\n");

      const text =
        `## ${c.displayName} (\`${c.connector}\`) credentials\n` +
        `Config message: \`${c.configMessage}\`\n\n` +
        `**Required fields:**\n${fieldLines.join("\n") || "  (none)"}\n` +
        (optLines.length ? `\n**Optional fields:**\n${optLines.join("\n")}\n` : "") +
        (c.supportsBaseUrl ? `\nSupports \`baseUrl\` override (optional).\n` : "") +
        `\n**connectorConfig template:**\n\`\`\`json\n${JSON.stringify(template, null, 2)}\n\`\`\`\n` +
        `\n**Suggested env vars:**\n\`\`\`\n${envLines.join("\n")}\n\`\`\`\n` +
        `\n**Sandbox test cards:**\n${cards}\n` +
        (c.notes ? `\n> ${c.notes}\n` : "") +
        (c.docsUrl ? `\nDocs: ${c.docsUrl}\n` : "");

      return result(text, {
        ok: true,
        connector: c.connector,
        displayName: c.displayName,
        configMessage: c.configMessage,
        requiredFields: req,
        optionalFields: opt,
        supportsBaseUrl: c.supportsBaseUrl,
        configTemplate: template,
        envVars: req.map((f) => envVarName(c.connector, f)),
        sandboxCards: c.sandboxCards,
        paymentMethods: c.paymentMethods,
        docsUrl: c.docsUrl,
        notes: c.notes,
      });
    },
  );
}
