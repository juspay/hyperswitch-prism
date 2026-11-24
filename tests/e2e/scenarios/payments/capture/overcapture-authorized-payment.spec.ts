import { expect } from "@playwright/test";
import { test } from "../../../framework/fixtures.js";
import { scenarioFromFile } from "../../catalog.js";
import { createCard, createCustomer, createReference, testReturnUrl } from "../../../test-data.js";

const scenario = scenarioFromFile(import.meta.url, {
  title: "Capture more than the authorized card-payment amount",
  description:
    "Authorize 6000 minor units with overcapture explicitly requested, then submit a 7000-unit capture.",
  verifies:
    "Verify overcapture is explicitly enabled on authorization, the connector accepts the larger capture amount, and HS records the connector-backed capture attempt.",
  requirements: {
    connectorAccount: {
      paymentMethodsEnabled: [{ payment_method: "card", payment_method_types: [{ payment_method_type: "credit" }] }],
    },
  },
});

test.use({ scenario });
test(scenario.title, async ({ connector, merchant, payments, report, expected }) => {
  const customer = createCustomer();
  const payment = await report.step("Authorize with overcapture enabled", async () => {
    const response = await payments.createPayment("overcapture-authorization", {
      profile_id: merchant.profileId,
      amount: 6000,
      currency: "USD",
      confirm: true,
      authentication_type: "no_three_ds",
      capture_method: "manual",
      payment_method: "card",
      payment_method_type: "credit",
      payment_method_data: { card: createCard(connector.card) },
      customer,
      enable_overcapture: true,
      description: "Synthetic overcapture authorization",
      metadata: { operation: "overcapture" },
      merchant_order_reference_id: createReference("order"),
      return_url: testReturnUrl,
    });
    await report.check(
      "Overcapture authorization fields",
      {
        actual: select(response, ["status", "amount", "currency", "amount_capturable", "amount_received", "capture_method", "connector", "error_code", "error_message"]),
        expected: expected("overcapture-authorization-fields", {
          status: "requires_capture", amount: 6000, currency: "USD", amount_capturable: 6000,
          amount_received: null, capture_method: "manual", connector: connector.id,
          error_code: null, error_message: null,
        }),
      },
      ({ actual, expected }) => expect(actual).toMatchObject(expected),
    );
    return response;
  });

  await report.step("Capture above the authorized amount", async () => {
    const response = await payments.capturePayment("overcapture", payment.payment_id, {
      amount_to_capture: 7000,
    });
    await report.check(
      "Overcapture response fields",
      {
        actual: select(response, ["status", "payment_id", "amount", "currency", "amount_capturable", "amount_received", "capture_method", "connector", "error_code", "error_message"]),
        expected: expected("overcapture-fields", {
          status: "succeeded", payment_id: payment.payment_id, amount: 6000, currency: "USD",
          amount_capturable: 0, amount_received: 7000, capture_method: "manual",
          connector: connector.id, error_code: null, error_message: null,
        }),
      },
      ({ actual, expected }) => expect(actual).toMatchObject(expected),
    );
  });

  const recorded = await payments.readRecordedPayment(payment.payment_id);
  await report.check(
    "Recorded overcapture outcome",
    {
      actual: recorded,
      expected: expected("overcapture-recorded-outcome", {
        gateway: "unified_connector_service", attemptStatus: "charged",
        connectorTransactionIdPresent: true, customerRecordExists: true,
      }),
    },
    ({ actual, expected }) => expect(actual).toEqual(expected),
  );
});

function select(response: Record<string, unknown>, fields: string[]) {
  return Object.fromEntries(fields.map((field) => [field, response[field]]));
}
