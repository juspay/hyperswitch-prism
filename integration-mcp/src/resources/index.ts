import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { RESOURCE_URIS, LLM_TXT_URL } from "../constants.js";
import { buildGuide } from "../guide.js";
import { paymentStatusList, refundStatusList, type StatusEntry } from "../status/statusMap.js";
import { fetchLlmTxt } from "../services/prism.js";

function statusTableMarkdown(): string {
  const fmt = (e: StatusEntry) => `| ${e.code} | ${e.name} | ${e.category} | ${e.meaning} |`;
  return (
    "# hyperswitch-prism status codes (numeric)\n\n" +
    "response.status is a NUMBER. Compare against types.PaymentStatus.* / types.RefundStatus.*.\n\n" +
    "## PaymentStatus\n| code | name | category | meaning |\n|---|---|---|---|\n" +
    paymentStatusList().map(fmt).join("\n") +
    "\n\n## RefundStatus\n| code | name | category | meaning |\n|---|---|---|---|\n" +
    refundStatusList().map(fmt).join("\n") +
    "\n\n> Soft declines = FAILURE (21) in the response body (no throw). Hard failures throw IntegrationError / ConnectorError / NetworkError.\n"
  );
}

export function registerResources(server: McpServer): void {
  // Integration guide (default: express, common flows).
  server.registerResource(
    "integration-guide",
    RESOURCE_URIS.guide,
    {
      title: "Prism integration guide",
      description: "End-to-end walkthrough for integrating hyperswitch-prism (default express, authorize+capture+refund).",
      mimeType: "text/markdown",
    },
    async (uri) => {
      const guide = buildGuide("express", ["authorize", "capture", "refund"]);
      return { contents: [{ uri: uri.href, mimeType: "text/markdown", text: guide.markdown }] };
    },
  );

  // Status table.
  server.registerResource(
    "status-table",
    RESOURCE_URIS.statusTable,
    {
      title: "Prism status code table",
      description: "Numeric PaymentStatus / RefundStatus codes with category and meaning.",
      mimeType: "text/markdown",
    },
    async (uri) => {
      return { contents: [{ uri: uri.href, mimeType: "text/markdown", text: statusTableMarkdown() }] };
    },
  );

  // Cached llm.txt.
  server.registerResource(
    "llm-txt",
    RESOURCE_URIS.llmTxt,
    {
      title: "Prism llm.txt (official LLM reference)",
      description: `Cached fetch of ${LLM_TXT_URL} — per-connector auth patterns and examples.`,
      mimeType: "text/plain",
    },
    async (uri) => {
      try {
        const { text } = await fetchLlmTxt();
        return { contents: [{ uri: uri.href, mimeType: "text/plain", text }] };
      } catch (err) {
        return {
          contents: [
            {
              uri: uri.href,
              mimeType: "text/plain",
              text: `Could not fetch llm.txt (${err instanceof Error ? err.message : String(err)}).\nSource: ${LLM_TXT_URL}`,
            },
          ],
        };
      }
    },
  );
}
