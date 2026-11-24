import { expect } from "@playwright/test";
import { test } from "../../../framework/fixtures.js";
import { scenarioFromFile } from "../../catalog.js";
import {
  createAddress,
  createBrowserInfo,
  createCard,
  createCustomer,
  createOrderItem,
  createReference,
  testReturnUrl,
} from "../../../test-data.js";

const scenario = scenarioFromFile(import.meta.url, {
  title: "Partially capture an authorized No3DS card payment",
  description:
    "Authorize a fully populated manual-capture payment for 6000 minor units, then capture 2500 and leave the remaining authorization uncaptured.",
  verifies:
    "Verify the initial authorization, exact partial-capture amount and response, payment identity, processor reference, and recorded partial-capture state.",
  requirements: {
    connectorAccount: {
      paymentMethodsEnabled: [{ payment_method: "card", payment_method_types: [{ payment_method_type: "credit" }] }],
    },
  },
});

test.use({ scenario });
test(scenario.title, async ({ connector, merchant, payments, report, expected }) => {
  const billingAddress = createAddress();
  const shippingAddress = createAddress({ line1: "2" });
  const customer = createCustomer();
  const phone = { number: customer.phone, country_code: customer.phone_country_code };
  const orderDetails = [createOrderItem({ quantity: 2, amount: 3000 })];
  const payment = await report.step("Authorize the payment", async () => {
    const response = await payments.createPayment("partial-capture-authorization", {
      profile_id: merchant.profileId,
      amount: 6000,
      currency: "USD",
      confirm: true,
      authentication_type: "no_three_ds",
      capture_method: "manual",
      payment_method: "card",
      payment_method_type: "credit",
      payment_method_data: { card: createCard(connector.card) },
      billing: { address: billingAddress, phone, email: customer.email },
      shipping: { address: shippingAddress, phone, email: customer.email },
      browser_info: createBrowserInfo(),
      customer,
      description: "Synthetic partial-capture authorization",
      metadata: { operation: "partial_capture" },
      merchant_order_reference_id: createReference("order"),
      order_details: orderDetails,
      return_url: testReturnUrl,
    });
    await report.check(
      "Partial-capture authorization fields",
      {
        actual: select(response, ["status", "amount", "currency", "amount_capturable", "amount_received", "capture_method", "connector", "merchant_id", "profile_id", "merchant_connector_id", "error_code", "error_message"]),
        expected: expected("partial-capture-authorization-fields", {
          status: "requires_capture", amount: 6000, currency: "USD", amount_capturable: 6000,
          amount_received: null, capture_method: "manual", connector: connector.id,
          merchant_id: merchant.merchantId, profile_id: merchant.profileId,
          merchant_connector_id: merchant.merchantConnectorId, error_code: null, error_message: null,
        }),
      },
      ({ actual, expected }) => expect(actual).toMatchObject(expected),
    );
    return response;
  });

  await report.step("Capture part of the authorization", async () => {
    const response = await payments.capturePayment("partial-capture", payment.payment_id, {
      amount_to_capture: 2500,
      statement_descriptor_suffix: "PARTIAL",
    });
    await report.check(
      "Partial-capture response fields",
      {
        actual: select(response, ["status", "payment_id", "amount", "currency", "amount_capturable", "amount_received", "capture_method", "connector", "error_code", "error_message"]),
        expected: expected("partial-capture-fields", {
          status: "partially_captured", payment_id: payment.payment_id, amount: 6000,
          currency: "USD", amount_capturable: 0, amount_received: 2500,
          capture_method: "manual", connector: connector.id, error_code: null, error_message: null,
        }),
      },
      ({ actual, expected }) => expect(actual).toMatchObject(expected),
    );
  });

  const recorded = await payments.readRecordedPayment(payment.payment_id);
  await report.check(
    "Recorded partial-capture outcome",
    {
      actual: recorded,
      expected: expected("partial-capture-recorded-outcome", {
        gateway: "unified_connector_service", attemptStatus: "partial_charged",
        connectorTransactionIdPresent: true, customerRecordExists: true,
      }),
    },
    ({ actual, expected }) => expect(actual).toEqual(expected),
  );
});

function select(response: Record<string, unknown>, fields: string[]) {
  return Object.fromEntries(fields.map((field) => [field, response[field]]));
}
