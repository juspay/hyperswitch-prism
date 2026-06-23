import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { statusReferenceShape } from "../schemas.js";
import { paymentStatusList, refundStatusList, lookupStatus, type StatusEntry } from "../status/statusMap.js";
import { result } from "../util.js";

function renderTable(entries: StatusEntry[]): string {
  const rows = entries.map(
    (e) => `| ${e.code} | ${e.name} | ${e.category} | ${e.meaning} |`,
  );
  return ["| code | name | category | meaning |", "|---|---|---|---|", ...rows].join("\n");
}

export function registerStatusReference(server: McpServer): void {
  server.registerTool(
    "prism_status_reference",
    {
      title: "Prism status code reference",
      description:
        "Explain hyperswitch-prism numeric status codes. response.status is a NUMBER (not a string). " +
        "Returns payment and/or refund status enums with their category (success/decline/pending/failure) and meaning. " +
        "Input: optional status_code (number) and optional kind ('payment'|'refund'). " +
        "Examples: { } lists all; { status_code: 8 } explains CHARGED; { kind: 'refund' } lists refund statuses. " +
        "Sourced from payment.proto enums (PaymentStatus / RefundStatus).",
      inputSchema: statusReferenceShape,
      annotations: { readOnlyHint: true, openWorldHint: false },
    },
    (args) => {
      const { status_code, kind } = args;

      if (typeof status_code === "number") {
        const entry = lookupStatus(status_code, kind);
        if (!entry) {
          return result(
            `No known status with code ${status_code}${kind ? ` for kind '${kind}'` : ""}. ` +
              `Codes are numeric; see the full table via prism_status_reference with no args.`,
            { ok: false, status_code, found: false },
          );
        }
        const text =
          `**${entry.name}** (code ${entry.code}, ${entry.kind})\n` +
          `Category: **${entry.category}**\n${entry.meaning}`;
        return result(text, { ok: true, status: entry });
      }

      const payment = paymentStatusList();
      const refund = refundStatusList();
      let text = "";
      const structured: Record<string, unknown> = { ok: true };
      if (kind !== "refund") {
        text += `### PaymentStatus (authorize / capture / void / sync)\n${renderTable(payment)}\n\n`;
        structured.paymentStatuses = payment;
      }
      if (kind !== "payment") {
        text += `### RefundStatus (refund / rsync)\n${renderTable(refund)}\n`;
        structured.refundStatuses = refund;
      }
      text +=
        "\n> Soft declines come back as status **FAILURE (21)** in the response body and do NOT throw. " +
        "Hard failures throw IntegrationError / ConnectorError / NetworkError.";
      return result(text, structured);
    },
  );
}
