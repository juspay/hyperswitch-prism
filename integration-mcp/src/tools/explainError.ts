import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { explainErrorShape } from "../schemas.js";
import { lookupStatus } from "../status/statusMap.js";
import { result } from "../util.js";

interface ErrorInfo {
  cause: string;
  fix: string;
  retriable: boolean;
}

const ERROR_CLASSES: Record<string, ErrorInfo> = {
  IntegrationError: {
    cause:
      "The SDK could not build a valid request at the FFI boundary — usually a bad/missing config field, a malformed " +
      "request shape, or native-library initialization failure. Thrown BEFORE the connector is contacted.",
    fix:
      "Do NOT retry. Fix the request/config: verify required fields with prism_connector_requirements, ensure secrets " +
      "are wrapped as { value: '...' }, and run prism_validate_config.",
    retriable: false,
  },
  ConnectorError: {
    cause:
      "The connector returned a response the SDK could not parse/transform into a normalized result (unexpected payload, " +
      "auth rejected at the gateway, unsupported operation for this account).",
    fix:
      "Do NOT blindly retry. Inspect error_code / error_message on the error. Check credentials are valid for the " +
      "environment (sandbox vs production) and that the connector supports this flow/payment method.",
    retriable: false,
  },
  NetworkError: {
    cause: "Transport-level failure: timeout, DNS failure, connection refused, or TLS error. The connector was never reached or did not answer in time.",
    fix: "Safe to retry with exponential backoff. Check connectivity, proxy/firewall, and timeout settings (totalTimeoutMs/connectTimeoutMs).",
    retriable: true,
  },
};

const ERROR_CODES: Record<string, ErrorInfo> = {
  MISSING_REQUIRED_FIELD: {
    cause: "A field the connector requires was absent from the request or config.",
    fix: "Run prism_connector_requirements for the connector and add the missing field. Re-validate with prism_validate_config.",
    retriable: false,
  },
  INVALID_CONFIGURATION: {
    cause: "The connectorConfig is structurally wrong — e.g. a secret not wrapped as { value }, wrong field name, or wrong connector key.",
    fix: "Run prism_validate_config to get the exact malformed field. Use prism_generate_config to regenerate a correct shape.",
    retriable: false,
  },
  CLIENT_INITIALIZATION: {
    cause: "The SDK failed to load/initialize its native FFI library — typically an unsupported platform/arch or a missing binary.",
    fix: "Run prism_doctor. The native lib ships for linux-x64 and macOS (arm64/x64). On unsupported platforms (e.g. linux-arm64) it cannot initialize.",
    retriable: false,
  },
  CONNECT_TIMEOUT: {
    cause: "The connection to the connector was not established within connectTimeoutMs.",
    fix: "Retry with backoff. Increase connectTimeoutMs, check egress/proxy, verify the connector base URL.",
    retriable: true,
  },
  CONNECT_TIMEOUT_EXCEEDED: {
    cause: "Connect timeout exceeded before a TCP/TLS connection was established.",
    fix: "Retry with backoff; check network egress and any proxy configuration.",
    retriable: true,
  },
  RESPONSE_TIMEOUT: {
    cause: "The connector accepted the connection but did not respond within responseTimeoutMs.",
    fix: "Retry idempotently (use the same merchantTransactionId). Increase responseTimeoutMs if the connector is slow.",
    retriable: true,
  },
  TOTAL_TIMEOUT_EXCEEDED: {
    cause: "The whole request exceeded totalTimeoutMs across connect + response.",
    fix: "Retry with backoff; raise totalTimeoutMs for slow connectors.",
    retriable: true,
  },
};

export function registerExplainError(server: McpServer): void {
  server.registerTool(
    "prism_explain_error",
    {
      title: "Explain a Prism error or decline",
      description:
        "Translate a hyperswitch-prism error into plain-language cause + concrete fix. Handles: error classes " +
        "(IntegrationError / ConnectorError / NetworkError), error codes (MISSING_REQUIRED_FIELD, INVALID_CONFIGURATION, " +
        "CLIENT_INITIALIZATION, CONNECT_TIMEOUT, RESPONSE_TIMEOUT, ...), and numeric status declines (e.g. status 21 = " +
        "FAILURE soft decline returned in the body, not thrown). Input: any of { errorClass, code, message, status, kind }. " +
        "Crucially clarifies that a FAILURE status is a soft decline in the response body — inspect the decline reason; " +
        "it does NOT throw.",
      inputSchema: explainErrorShape,
      annotations: { readOnlyHint: true, openWorldHint: false },
    },
    (args) => {
      const sections: string[] = [];
      const structured: Record<string, unknown> = { ok: true };

      if (args.errorClass) {
        const info = ERROR_CLASSES[args.errorClass];
        if (info) {
          sections.push(
            `### ${args.errorClass}\n**Cause:** ${info.cause}\n**Fix:** ${info.fix}\n**Retriable:** ${info.retriable ? "yes (backoff)" : "no"}`,
          );
          structured.errorClass = { name: args.errorClass, ...info };
        }
      }

      // Try to match a code, either explicit or scraped from the message.
      const codeFromMessage = args.message
        ? Object.keys(ERROR_CODES).find((k) => args.message!.toUpperCase().includes(k))
        : undefined;
      const code = args.code?.toUpperCase() ?? codeFromMessage;
      if (code && ERROR_CODES[code]) {
        const info = ERROR_CODES[code];
        sections.push(
          `### Code: ${code}\n**Cause:** ${info.cause}\n**Fix:** ${info.fix}\n**Retriable:** ${info.retriable ? "yes (backoff)" : "no"}`,
        );
        structured.code = { name: code, ...info };
      } else if (args.code) {
        sections.push(
          `### Code: ${args.code}\nNot a recognized SDK error code. If it came from the connector, inspect error_message ` +
            `on the ConnectorError and check the connector's own docs.`,
        );
        structured.code = { name: args.code, recognized: false };
      }

      if (typeof args.status === "number") {
        const entry = lookupStatus(args.status, args.kind);
        if (entry) {
          const isSoftDecline = entry.name === "FAILURE";
          sections.push(
            `### Status ${entry.code} = ${entry.name} (${entry.category})\n${entry.meaning}` +
              (isSoftDecline
                ? `\n\n**Important:** this is a soft decline returned IN the response body — it does NOT throw. ` +
                  `Common reasons: card declined by issuer, currency/payment-method not enabled on the sandbox account, ` +
                  `insufficient funds, or AVS/CVC failure. Inspect the response's error_message / decline reason field.`
                : ""),
          );
          structured.status = entry;
        } else {
          sections.push(`### Status ${args.status}\nUnknown status code. See prism_status_reference for the full table.`);
          structured.status = { code: args.status, recognized: false };
        }
      }

      if (sections.length === 0) {
        return result(
          "Provide at least one of: errorClass, code, message, or status. " +
            "Example: { status: 21 } or { errorClass: 'NetworkError', code: 'CONNECT_TIMEOUT' }.",
          { ok: false },
        );
      }

      return result(sections.join("\n\n"), structured);
    },
  );
}
