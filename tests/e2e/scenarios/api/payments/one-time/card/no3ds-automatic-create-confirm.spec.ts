import { test } from "@playwright/test";
import { connectors } from "../../../../../connectors/index.js";
import { runScenario } from "../../../../../framework/runner.js";
import {
  supports,
  type PaymentScenario,
} from "../../../../../framework/scenario.js";

// Each test provisions an isolated merchant, connector account, and routing rule.
const scenario = {
  id: "no3ds-automatic-create-confirm",
  title: "One-time card payment — No3DS, automatic capture",
  description:
    "Create and confirm in one server API request, retrieve the successful payment, and verify UCS routing and a recorded processor reference.",
  paymentMethod: "card",
  authentication: "no_three_ds",
  captureMethod: "automatic",
  steps: [
    {
      name: "POST /payments — create and confirm with automatic capture",
      action: "createPayment",
      saveAs: "payment",
      request: {
        amount: 6000,
        currency: "USD",
        confirm: true,
        authentication_type: "no_three_ds",
        capture_method: "automatic",
        payment_method: "card",
        payment_method_type: "credit",
        payment_method_data: { card: "$connector.card" },
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
      },
      expect: {
        status: { oneOf: ["succeeded", "processing"] },
        connector: { from: "connector.id" },
        merchant_connector_id: { from: "merchant.merchantConnectorId" },
        next_action: null,
        mandate_id: null,
      },
    },
    {
      name: "GET /payments/{payment_id} — wait for succeeded and verify payment fields",
      action: "retrievePayment",
      payment: "payment",
      request: { force_sync: true },
      waitForStatus: "succeeded",
      expect: {
        status: "succeeded",
        connector: { from: "connector.id" },
        merchant_connector_id: { from: "merchant.merchantConnectorId" },
        authentication_type: "no_three_ds",
        amount: 6000,
        currency: "USD",
        amount_received: 6000,
        setup_future_usage: null,
        mandate_id: null,
      },
    },
    {
      name: "Verify recorded UCS gateway and processor transaction reference",
      action: "verifyPaymentRoute",
      payment: "payment",
      request: { attempt_status: "charged" },
      expect: {
        gateway: "unified_connector_service",
        processorReferenceRecorded: true,
      },
    },
  ],
} as const satisfies PaymentScenario;

for (const connector of connectors.filter((item) => supports(item, scenario))) {
  test(`${connector.id} / ${scenario.title} @one-time @api @card`, async ({}, info) => {
    await runScenario(connector, scenario, info);
  });
}
