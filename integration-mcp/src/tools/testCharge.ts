import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { testChargeShape } from "../schemas.js";
import { getConnector, suggestConnectors, currencyCode, ENUMS } from "../data/connectors.js";
import { extractInner, validateInner } from "../config.js";
import { loadSdk, probeNative, tryNativeInit, normalizeError } from "../services/prism.js";
import { lookupStatus } from "../status/statusMap.js";
import { CONFIG_ENV_VAR } from "../constants.js";
import { result, errorResult, scrubSecrets, unknownToMessage } from "../util.js";

/** Collect secret string values from a parsed config so they can be scrubbed from output. */
function collectSecrets(obj: unknown, acc: string[]): void {
  if (!obj || typeof obj !== "object") return;
  if (Array.isArray(obj)) {
    for (const v of obj) collectSecrets(v, acc);
    return;
  }
  const rec = obj as Record<string, unknown>;
  if (typeof rec.value === "string" && Object.keys(rec).length === 1) {
    acc.push(rec.value);
    return;
  }
  for (const v of Object.values(rec)) collectSecrets(v, acc);
}

export function registerTestCharge(server: McpServer): void {
  server.registerTool(
    "prism_test_charge",
    {
      title: "Run a Prism sandbox test charge",
      description:
        "Run a REAL sandbox authorize to prove the integration works end-to-end. Reads credentials ONLY from the " +
        `${CONFIG_ENV_VAR} environment variable (never from tool args) and forces testMode/SANDBOX. Hard-fails if ` +
        "test_mode is not true. Input: { connector, amount (minor units), currency, card?, captureMethod, test_mode }. " +
        "Returns a normalized PaymentResult: { success, statusName, category, connectorTransactionId, declineReason? }. " +
        "All secrets are redacted from output.",
      inputSchema: testChargeShape,
      annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: true, idempotentHint: false },
    },
    async (args) => {
      // 1. Enforce sandbox-only.
      if (args.test_mode !== true) {
        return errorResult(
          "Refusing to run: prism_test_charge only ever performs SANDBOX charges and requires test_mode === true.",
          { ran: false, reason: "test_mode_not_true" },
        );
      }

      const c = getConnector(args.connector);
      if (!c) {
        return errorResult(
          `Unknown connector '${args.connector}'. Did you mean: ${suggestConnectors(args.connector).join(", ") || "(none)"}?`,
          { ran: false, connector: args.connector },
        );
      }

      // 2. Native env must be able to run — check the file-level probe AND an
      //    actual native load (catches glibc/loader issues a probe can't see).
      const probe = probeNative();
      if (!probe.willLoad) {
        return errorResult(`Cannot run a test charge: ${probe.reason} (run prism_doctor for details).`, {
          ran: false,
          reason: "native_unavailable",
          native: probe,
        });
      }
      const nativeInit = await tryNativeInit();
      if (!nativeInit.ok) {
        return errorResult(
          `Cannot run a test charge: native library failed to load${nativeInit.code ? ` (${nativeInit.code})` : ""}: ` +
            `${nativeInit.error}. Run prism_doctor for the fix.`,
          { ran: false, reason: "native_load_failed", nativeInit },
        );
      }

      // 3. Currency.
      const currencyNum = currencyCode(args.currency);
      if (currencyNum === undefined) {
        return errorResult(`Unknown ISO 4217 currency '${args.currency}'.`, { ran: false });
      }

      // 4. Read + validate credentials from env (never from args).
      const rawEnv = process.env[CONFIG_ENV_VAR];
      if (!rawEnv) {
        return errorResult(
          `${CONFIG_ENV_VAR} is not set. Set it to your sandbox connectorConfig JSON (see prism_generate_config), then retry.`,
          { ran: false, reason: "missing_env" },
        );
      }
      let parsedConfig: Record<string, unknown>;
      try {
        parsedConfig = JSON.parse(rawEnv) as Record<string, unknown>;
      } catch {
        return errorResult(`${CONFIG_ENV_VAR} is not valid JSON.`, { ran: false, reason: "bad_json" });
      }

      const inner = extractInner(c.connector, parsedConfig);
      if (!inner) {
        return errorResult(
          `${CONFIG_ENV_VAR} does not contain a config for '${c.connector}'. Expected connectorConfig.${c.connector}.`,
          { ran: false, reason: "missing_connector_config" },
        );
      }
      const issues = validateInner(c, inner);
      if (issues.length > 0) {
        return errorResult(
          `Credentials in ${CONFIG_ENV_VAR} are invalid for '${c.connector}': ` +
            issues.map((i) => `${i.field} (${i.problem})`).join("; "),
          { ran: false, reason: "invalid_config", issues },
        );
      }

      const secrets: string[] = [];
      collectSecrets(inner, secrets);

      // 5. Build the SDK client and a sandbox authorize request.
      const card = args.card ?? {
        number: c.sandboxCards[0]?.number ?? "4111111111111111",
        expMonth: c.sandboxCards[0]?.expMonth ?? "12",
        expYear: c.sandboxCards[0]?.expYear ?? "2030",
        cvc: c.sandboxCards[0]?.cvc ?? "123",
        holderName: "Test Customer",
      };

      try {
        const sdk = await loadSdk();
        const PaymentClient = sdk.PaymentClient as new (cfg: unknown) => {
          authorize: (req: unknown) => Promise<{ status: number; connectorTransactionId?: string; errorMessage?: string }>;
        };
        const types = sdk.types as { Environment: Record<string, number>; CaptureMethod: Record<string, number>; AuthenticationType: Record<string, number> };

        const client = new PaymentClient({
          connectorConfig: { [c.connector]: inner },
          options: { environment: types.Environment.SANDBOX },
        });

        const startedAt = Date.now();
        const res = await client.authorize({
          merchantTransactionId: `prism_mcp_test_${startedAt}`,
          amount: { minorAmount: args.amount, currency: currencyNum },
          captureMethod: types.CaptureMethod[args.captureMethod],
          paymentMethod: {
            card: {
              cardNumber: { value: card.number },
              cardExpMonth: { value: card.expMonth },
              cardExpYear: { value: card.expYear },
              cardCvc: { value: card.cvc },
              cardHolderName: { value: card.holderName ?? "Test Customer" },
            },
          },
          address: { billingAddress: {} },
          authType: types.AuthenticationType.NO_THREE_DS,
          orderDetails: [],
          testMode: true,
        });
        const latencyMs = Date.now() - startedAt;

        const entry = lookupStatus(res.status, "payment");
        const charged = ENUMS.PaymentStatus.CHARGED;
        const authorized = ENUMS.PaymentStatus.AUTHORIZED;
        const success = args.captureMethod === "AUTOMATIC" ? res.status === charged : res.status === authorized;
        const declined = entry?.category === "decline";
        const declineReason = declined ? scrubSecrets(res.errorMessage ?? "Soft decline (no reason provided)", secrets) : undefined;

        const text =
          `${success ? "✅" : declined ? "⚠️" : "ℹ️"} Sandbox authorize on **${c.displayName}** → ` +
          `**${entry?.name ?? res.status}** (status ${res.status}, ${entry?.category ?? "unknown"}) in ${latencyMs}ms.\n` +
          (res.connectorTransactionId ? `Transaction: ${res.connectorTransactionId}\n` : "") +
          (declineReason ? `Decline reason: ${declineReason}\n` : "") +
          (success
            ? args.captureMethod === "MANUAL"
              ? "\nAuthorized — call capture next to charge it."
              : "\nCharged successfully. Your integration works end-to-end."
            : declined
              ? "\nSoft decline (returned in the response body, not thrown). Check the decline reason and sandbox account settings."
              : "");

        return result(text, {
          ok: true,
          ran: true,
          connector: c.connector,
          success,
          declined,
          status: res.status,
          statusName: entry?.name ?? null,
          category: entry?.category ?? null,
          connectorTransactionId: res.connectorTransactionId ?? null,
          declineReason: declineReason ?? null,
          latencyMs,
        });
      } catch (err) {
        const norm = normalizeError(err);
        norm.message = scrubSecrets(norm.message, secrets);
        return errorResult(
          `Test charge threw ${norm.errorClass}${norm.code ? ` (${norm.code})` : ""}: ${norm.message}\n` +
            `Run prism_explain_error with errorClass='${norm.errorClass}' for guidance.`,
          { ran: true, threw: true, error: { ...norm, message: scrubSecrets(unknownToMessage(err), secrets) } },
        );
      }
    },
  );
}
