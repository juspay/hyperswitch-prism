import { expect } from "@playwright/test";
import { test } from "../../../framework/fixtures.js";
import { scenarioFromFile } from "../../catalog.js";
import {
  createAddress,
  createBrowserInfo,
  createCard,
  createCustomer,
  createReference,
  testReturnUrl,
} from "../../../test-data.js";

const scenario = scenarioFromFile(import.meta.url, {
  title: "Partially refund a captured No3DS card payment",
  description:
    "Capture a 6000-unit payment, refund 2500 units, and preserve the remaining captured amount.",
  verifies:
    "Verify the captured payment, exact partial-refund amount and identity, processor refund reference, and recorded refund relationship.",
  requirements: {
    connectorAccount: {
      paymentMethodsEnabled: [
        {
          payment_method: "card",
          payment_method_types: [{ payment_method_type: "credit" }],
        },
      ],
    },
  },
});

test.use({ scenario });
test(
  scenario.title,
  async ({ connector, merchant, payments, refunds, report, expected }) => {
    const customer = createCustomer();
    const payment = await report.step("Capture the payment", async () => {
      const response = await payments.createPayment("partial-refund-payment", {
        profile_id: merchant.profileId,
        amount: 6000,
        currency: "USD",
        confirm: true,
        authentication_type: "no_three_ds",
        capture_method: "automatic",
        payment_method: "card",
        payment_method_type: "credit",
        payment_method_data: { card: createCard(connector.card) },
        billing: { address: createAddress(), email: customer.email },
        browser_info: createBrowserInfo(),
        customer,
        description: "Synthetic order for partial refund",
        metadata: { operation: "partial_refund" },
        merchant_order_reference_id: createReference("order"),
        return_url: testReturnUrl,
      });
      await report.check(
        "Partial-refund payment fields",
        {
          actual: select(response, [
            "status",
            "amount",
            "currency",
            "amount_capturable",
            "amount_received",
            "capture_method",
            "connector",
            "error_code",
            "error_message",
          ]),
          expected: expected("partial-refund-payment-fields", {
            status: "succeeded",
            amount: 6000,
            currency: "USD",
            amount_capturable: 0,
            amount_received: 6000,
            capture_method: "automatic",
            connector: connector.id,
            error_code: null,
            error_message: null,
          }),
        },
        ({ actual, expected }) => expect(actual).toMatchObject(expected),
      );
      return response;
    });

    const refund = await report.step("Refund part of the payment", async () => {
      const response = await refunds.createRefund("partial-refund", {
        payment_id: payment.payment_id,
        amount: 2500,
        reason: "PARTIAL CUSTOMER REFUND",
      });
      await report.check(
        "Partial-refund submission fields",
        {
          actual: select(response, [
            "status",
            "amount",
            "currency",
            "payment_id",
            "error_code",
            "error_message",
          ]),
          expected: expected("partial-refund-submission-fields", {
            status: "succeeded",
            amount: 2500,
            currency: "USD",
            payment_id: payment.payment_id,
            error_code: null,
            error_message: null,
          }),
        },
        ({ actual, expected }) => expect(actual).toMatchObject(expected),
      );
      return response;
    });

    const recorded = await refunds.readRecordedRefund(
      refund.refund_id,
      payment.payment_id,
    );
    await report.check(
      "Recorded partial-refund outcome",
      {
        actual: recorded,
        expected: expected("partial-refund-recorded-outcome", {
          status: "success",
          connectorRefundIdPresent: true,
          paymentIdMatches: true,
        }),
      },
      ({ actual, expected }) => expect(actual).toEqual(expected),
    );
  },
);

function select(response: Record<string, unknown>, fields: string[]) {
  return Object.fromEntries(fields.map((field) => [field, response[field]]));
}
