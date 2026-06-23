import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { validateConfigShape } from "../schemas.js";
import { getConnector, suggestConnectors, requiredFields } from "../data/connectors.js";
import { extractInner, validateInner } from "../config.js";
import { result, errorResult } from "../util.js";

export function registerValidateConfig(server: McpServer): void {
  server.registerTool(
    "prism_validate_config",
    {
      title: "Validate a Prism connector config",
      description:
        "Validate a candidate connector config against the connector's exact field spec (from payment.proto). " +
        "Checks every required field is present, secret fields are wrapped as { value: '...' }, types are correct, " +
        "and flags unknown fields. Input: { connector, config }. The config may be the full " +
        "{ connectorConfig: { <name>: {...} } } shape or just the inner per-connector object. " +
        "Returns pass/fail with a precise list of problems and how to fix each.",
      inputSchema: validateConfigShape,
      annotations: { readOnlyHint: true, openWorldHint: false },
    },
    (args) => {
      const c = getConnector(args.connector);
      if (!c) {
        return errorResult(
          `Unknown connector '${args.connector}'. Did you mean: ${suggestConnectors(args.connector).join(", ") || "(none)"}?`,
          { connector: args.connector },
        );
      }

      const inner = extractInner(c.connector, args.config);
      if (!inner) {
        return errorResult(
          `Could not find a config object for '${c.connector}'. Expected { connectorConfig: { ${c.connector}: {...} } } ` +
            `or the inner object directly.`,
          { connector: c.connector, valid: false },
        );
      }

      const issues = validateInner(c, inner);
      const valid = issues.length === 0;
      const reqNames = requiredFields(c).map((f) => f.name);

      if (valid) {
        return result(
          `✅ Config for **${c.displayName}** (\`${c.connector}\`) is valid. ` +
            `All required fields present: ${reqNames.join(", ") || "(none)"}.`,
          { ok: true, connector: c.connector, valid: true, issues: [] },
        );
      }

      const lines = issues.map((i) => `- **${i.field}**: ${i.problem}\n    fix: ${i.fix}`);
      const text =
        `❌ Config for **${c.displayName}** (\`${c.connector}\`) has ${issues.length} problem(s):\n` +
        `${lines.join("\n")}\n\n` +
        `Required fields for ${c.connector}: ${reqNames.join(", ") || "(none)"}.`;
      return result(text, { ok: true, connector: c.connector, valid: false, issues });
    },
  );
}
