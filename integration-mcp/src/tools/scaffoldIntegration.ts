import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { scaffoldIntegrationShape } from "../schemas.js";
import { getConnector, suggestConnectors } from "../data/connectors.js";
import { scaffold, type ScaffoldFile } from "../scaffold/index.js";
import { result, errorResult } from "../util.js";

export function registerScaffoldIntegration(server: McpServer): void {
  server.registerTool(
    "prism_scaffold_integration",
    {
      title: "Scaffold a Prism integration",
      description:
        "Generate copy-pasteable integration code (returns { path, contents } files — does NOT write to disk). " +
        "Input: { framework, connector, flows[], language: 'ts'|'js', captureMethod: 'AUTOMATIC'|'MANUAL' }. " +
        "Emits: an env-driven PaymentClient module, framework-agnostic flow handlers with the real SDK request shapes " +
        "and NUMERIC status checks (CHARGED=8, AUTHORIZED=6, VOIDED=11, REFUND_SUCCESS=4), framework route wiring " +
        "(express/next/nestjs/fastify/node), and a .env.example. The connectorConfig field names come from payment.proto " +
        "(never guessed). The calling agent writes these files into the merchant's repo.",
      inputSchema: scaffoldIntegrationShape,
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

      const files: ScaffoldFile[] = scaffold({
        connector: c,
        framework: args.framework,
        language: args.language,
        flows: args.flows,
        captureMethod: args.captureMethod,
      });

      const text =
        `Scaffolded ${files.length} file(s) for **${c.displayName}** on **${args.framework}** ` +
        `(${args.language}, ${args.captureMethod} capture, flows: ${args.flows.join(", ")}).\n\n` +
        files
          .map((f) => `### ${f.path}\n\`\`\`${args.language === "ts" ? "ts" : "js"}\n${f.contents}\`\`\``)
          .join("\n\n") +
        `\n\nNext: set the env vars from .env.example, then run \`prism_test_charge\` to confirm a sandbox CHARGED.`;

      return result(text, {
        ok: true,
        connector: c.connector,
        framework: args.framework,
        language: args.language,
        captureMethod: args.captureMethod,
        flows: args.flows,
        files,
      });
    },
  );
}
