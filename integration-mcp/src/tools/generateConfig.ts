import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { generateConfigShape } from "../schemas.js";
import { getConnector, suggestConnectors, requiredFields, optionalFields, envVarName } from "../data/connectors.js";
import { CONFIG_ENV_VAR } from "../constants.js";
import { result, errorResult } from "../util.js";

export function registerGenerateConfig(server: McpServer): void {
  server.registerTool(
    "prism_generate_config",
    {
      title: "Generate Prism connector config + .env",
      description:
        "Generate the PRISM_CONNECTOR_CONFIG JSON and matching .env / .env.example entries for a connector. " +
        "Secrets are referenced from environment variables — never hard-coded into the JSON. " +
        "Input: { connector, values?: { <fieldName>: <value> }, environment?: 'sandbox'|'production' }. " +
        "If values are omitted, placeholders are emitted. Output: an env-driven connectorConfig object, a JSON string " +
        "suitable for the PRISM_CONNECTOR_CONFIG env var, and .env/.env.example lines.",
      inputSchema: generateConfigShape,
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

      const fields = [...requiredFields(c), ...optionalFields(c)];
      const values = args.values ?? {};

      // .env lines (secrets + plain values both live in env).
      const envLines: string[] = [];
      const envExampleLines: string[] = [];
      // The runtime connectorConfig that reads from process.env.
      const innerRuntime: Record<string, unknown> = {};
      // A literal JSON form for PRISM_CONNECTOR_CONFIG (values inlined from provided or placeholder).
      const innerLiteral: Record<string, unknown> = {};

      for (const f of fields) {
        if (f.isBaseUrl) continue;
        const isOptional = !f.required;
        const provided = values[f.name];
        if (isOptional && provided === undefined) continue;
        const envName = envVarName(c.connector, f);
        const exampleVal = c.fieldDocs[f.name]?.example ?? "";
        envLines.push(`${envName}=${provided ?? exampleVal}`);
        envExampleLines.push(`${envName}=${exampleVal || (f.secret ? "your_secret_here" : "value")}`);

        const literalVal = provided ?? `<${f.name}>`;
        if (f.secret) {
          innerRuntime[f.name] = { value: `\${${envName}}` };
          innerLiteral[f.name] = { value: literalVal };
        } else {
          innerRuntime[f.name] = `\${${envName}}`;
          innerLiteral[f.name] = literalVal;
        }
      }

      const literalConfig = { connectorConfig: { [c.connector]: innerLiteral } };
      const runtimeSnippet =
        `// Build PRISM_CONNECTOR_CONFIG from env at startup (never commit secrets):\n` +
        `const PRISM_CONNECTOR_CONFIG = JSON.stringify({\n` +
        `  connectorConfig: {\n` +
        `    ${c.connector}: {\n` +
        fields
          .filter((f) => !f.isBaseUrl && (f.required || values[f.name] !== undefined))
          .map((f) =>
            f.secret
              ? `      ${f.name}: { value: process.env.${envVarName(c.connector, f)} },`
              : `      ${f.name}: process.env.${envVarName(c.connector, f)},`,
          )
          .join("\n") +
        `\n    }\n  }\n});`;

      const text =
        `## ${c.displayName} config (${args.environment})\n\n` +
        `**.env** (do NOT commit):\n\`\`\`bash\n${envLines.join("\n")}\n\`\`\`\n\n` +
        `**.env.example** (safe to commit):\n\`\`\`bash\n${envExampleLines.join("\n")}\n\`\`\`\n\n` +
        `**${CONFIG_ENV_VAR}** value (placeholders — replace via env, don't paste real secrets):\n` +
        `\`\`\`json\n${JSON.stringify(literalConfig, null, 2)}\n\`\`\`\n\n` +
        `**Build it from env in code:**\n\`\`\`ts\n${runtimeSnippet}\n\`\`\`\n`;

      return result(text, {
        ok: true,
        connector: c.connector,
        environment: args.environment,
        envExample: envExampleLines,
        configEnvVar: CONFIG_ENV_VAR,
        literalConfig,
        runtimeConfig: { connectorConfig: { [c.connector]: innerRuntime } },
      });
    },
  );
}
