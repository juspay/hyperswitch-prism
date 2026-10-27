import { test } from "../../../../../framework/fixtures.js";
import { connectors } from "../../../../../connectors/index.js";

const scenario = {
  id: "no3ds-automatic-create-confirm",
  title: "One-time card payment — No3DS, automatic capture",
  description:
    "Create and confirm in one server API request, retrieve the successful payment, and verify the recorded UCS gateway and processor reference.",
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
    test(scenario.title, async ({ connector, merchant, payments, checks }) => {
      // Setup provides an isolated merchant, API key, connector account, and routing.
      // 1. POST /payments: create and confirm together, with automatic capture.
      const createdPayment = await payments.createPayment({
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
          address: {
            first_name: "Test",
            last_name: "Customer",
            line1: "1 Test Street",
            city: "San Francisco",
            state: "CA",
            zip: "94107",
            country: "US",
          },
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
        email: "test@example.com",
        return_url: "https://example.com",
      });

      await checks.oneOf("Confirmation status", createdPayment.status, [
        "succeeded",
        "processing",
      ]);
      await checks.fields("Confirmation fields", createdPayment, {
        connector: connector.id,
        merchant_connector_id: merchant.merchantConnectorId,
        next_action: null,
        mandate_id: null,
      });

      // 2. GET /payments/{id}?force_sync=true: poll until succeeded (at most 90s).
      // The whole response remains available; any field can be used by later calls.
      const payment = await payments.waitForPaymentStatus(
        createdPayment.payment_id,
        { force_sync: true },
        "succeeded",
      );
      await checks.fields("Payment result", payment, {
        payment_id: createdPayment.payment_id,
        status: "succeeded",
        connector: connector.id,
        merchant_connector_id: merchant.merchantConnectorId,
        authentication_type: "no_three_ds",
        amount: 6000,
        currency: "USD",
        amount_received: 6000,
        setup_future_usage: null,
        mandate_id: null,
      });

      // 3. Read recorded facts for this payment and its active attempt.
      // This does not trigger or prove a fresh processor PSync call.
      const recorded = await payments.readRecordedPayment(payment.payment_id);
      await checks.fields("Recorded payment outcome", recorded, {
        gateway: "unified_connector_service",
        attemptStatus: "charged",
        connectorTransactionIdPresent: true,
      });
    });
  });
}
