import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { listConnectorsShape } from "../schemas.js";
import { listConnectors, searchConnectors, CONNECTOR_COUNT, type Connector } from "../data/connectors.js";
import { requiredFields } from "../data/connectors.js";
import { result } from "../util.js";

function summarize(c: Connector) {
  return {
    connector: c.connector,
    displayName: c.displayName,
    paymentMethods: c.paymentMethods,
    requiredFields: requiredFields(c).map((f) => f.name),
    supportsBaseUrl: c.supportsBaseUrl,
  };
}

export function registerListConnectors(server: McpServer): void {
  server.registerTool(
    "prism_list_connectors",
    {
      title: "List Prism connectors",
      description:
        "List the payment processors/connectors supported by hyperswitch-prism (derived from payment.proto, " +
        `${CONNECTOR_COUNT} connectors). Optional 'search' does a case-insensitive substring match over machine ` +
        "name and display name. Each entry includes the machine name (the connectorConfig key), display name, " +
        "supported payment methods, and the required credential field names. " +
        "Examples: { } lists all; { search: 'pay' } finds paypal, payme, paytm, etc.",
      inputSchema: listConnectorsShape,
      annotations: { readOnlyHint: true, openWorldHint: false },
    },
    (args) => {
      const matches = args.search ? searchConnectors(args.search) : listConnectors();
      const summarized = matches.map(summarize);
      const lines = summarized.map(
        (s) =>
          `- **${s.connector}** (${s.displayName}) — methods: ${s.paymentMethods.join(", ")}; ` +
          `creds: ${s.requiredFields.join(", ") || "none"}`,
      );
      const header = args.search
        ? `${matches.length} connector(s) matching "${args.search}" (of ${CONNECTOR_COUNT}):`
        : `${CONNECTOR_COUNT} supported connectors:`;
      const text =
        matches.length === 0
          ? `No connectors match "${args.search}". Try a shorter query or run with no search.`
          : `${header}\n${lines.join("\n")}\n\nUse prism_connector_requirements for a connector's exact credential shape.`;
      return result(text, { ok: true, count: matches.length, total: CONNECTOR_COUNT, connectors: summarized });
    },
  );
}
