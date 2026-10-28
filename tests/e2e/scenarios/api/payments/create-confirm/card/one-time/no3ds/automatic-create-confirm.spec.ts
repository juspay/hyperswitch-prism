import { test } from "../../../../../../../framework/fixtures.js";
import { connectors } from "../../../../../../../connectors/index.js";

const scenario = {
  id: "no3ds-automatic-create-confirm",
  title: "One-time card payment — No3DS, automatic capture",
  description:
    "Create and confirm a one-time No3DS card payment with automatic capture in one API request. Verify succeeded status, the full amount received, and a recorded charged attempt through UCS with a connector transaction ID.",
  verifies:
    "Verify full automatic capture with no amount left capturable, merchant and profile ownership, billing and contact details, one attempt, no errors or mandate setup, and the recorded UCS gateway and connector transaction ID.",
  paymentMethodsEnabled: [
    {
      payment_method: "card",
      payment_method_types: [{ payment_method_type: "credit" }],
    },
  ],
};

for (const connector of connectors) {
  test.describe(connector.id, () => {
    test.use({ connector, scenario });
    test(scenario.title, async ({ connector, merchant, payments, checks, report }) => {
      const billingAddress = {
        first_name: "Test",
        last_name: "Customer",
        line1: "1 Test Street",
        city: "San Francisco",
        state: "CA",
        zip: "94107",
        country: "US",
      };
      const email = "test@example.com";
      const returnUrl = "https://example.com";
      // Setup provides an isolated merchant, API key, connector account, and routing.
      // 1. POST /payments: create and confirm together, with automatic capture.
      const payment = await report.step("Create and confirm payment", async () => {
        const response = await payments.createPayment({
          profile_id: merchant.profileId,
          amount: 6000,
          currency: "USD",
          confirm: true,
          authentication_type: "no_three_ds",
          capture_method: "automatic",
          payment_method: "card",
          payment_method_type: "credit",
          payment_method_data: { card: connector.card },
          billing: {
            address: billingAddress,
          },
          browser_info: {
            accept_header:
              "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            language: "en-US",
            screen_height: 1080,
            screen_width: 1920,
            color_depth: 24,
            user_agent:
              "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/130.0.0.0 Safari/537.36",
            time_zone: 0,
            java_enabled: false,
            ip_address: "192.0.2.1",
          },
          email,
          return_url: returnUrl,
        });

        await checks.fields("Confirmation fields", response, {
          status: "succeeded",
          error_code: null,
          error_message: null,
          error_details: null,
          issuer_error_code: null,
          issuer_error_message: null,
          merchant_id: merchant.merchantId,
          profile_id: merchant.profileId,
          connector: connector.id,
          merchant_connector_id: merchant.merchantConnectorId,
          capture_method: "automatic",
          payment_method: "card",
          payment_method_type: "credit",
          authentication_type: "no_three_ds",
          amount: 6000,
          net_amount: 6000,
          amount_capturable: 0,
          currency: "USD",
          amount_received: 6000,
          setup_future_usage: null,
          next_action: null,
          mandate_id: null,
          mandate_data: null,
          connector_mandate_id: null,
          attempt_count: 1,
          email,
        });
        const billing = response.billing;
        if (
          billing === null ||
          typeof billing !== "object" ||
          !("address" in billing)
        ) {
          throw new Error("Payment response must contain a billing address");
        }
        await checks.fields("Billing address", billing.address, billingAddress);

        // Compare URL values after standard URL normalization.
        const actualReturnUrl = response.return_url;
        await checks.fields(
          "Return URL",
          {
            return_url: typeof actualReturnUrl === "string"
              ? new URL(actualReturnUrl).href
              : null,
          },
          { return_url: new URL(returnUrl).href },
        );

        // Report presence and validity rather than exposing generated references.
        await checks.fields(
          "Generated payment references",
          {
            paymentIdPresent: typeof response.payment_id === "string"
              && response.payment_id.trim().length > 0,
            connectorTransactionIdPresent:
              typeof response.connector_transaction_id === "string"
              && response.connector_transaction_id.trim().length > 0,
          },
          { paymentIdPresent: true, connectorTransactionIdPresent: true },
        );
        await checks.fields(
          "Payment timestamps",
          {
            createdValid: typeof response.created === "string"
              && Number.isFinite(Date.parse(response.created)),
            modifiedValid: typeof response.modified_at === "string"
              && Number.isFinite(Date.parse(response.modified_at)),
          },
          { createdValid: true, modifiedValid: true },
        );
        return response;
      });

      // 2. Read recorded facts for this payment and its active attempt.
      // This does not trigger or prove a fresh processor PSync call.
      await report.step("Verify recorded payment outcome", async () => {
        const recorded = await payments.readRecordedPayment(payment.payment_id);
        await checks.fields("Recorded payment outcome", recorded, {
          gateway: "unified_connector_service",
          attemptStatus: "charged",
          connectorTransactionIdPresent: true,
        });
      });
    });
  });
}
