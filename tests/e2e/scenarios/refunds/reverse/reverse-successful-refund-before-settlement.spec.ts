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
  title: "Reverse a successful refund before settlement",
  description:
    "Capture a payment, create a successful refund, then invoke the HS refund-reversal endpoint while the processor can still void that refund.",
  verifies:
    "Verify the payment and refund succeed, the reversal preserves refund identity, and HS records the processor reversal result and reference.",
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
    const payment = await report.step("Capture the refundable payment", () =>
      payments.createPayment("refund-reversal-payment", {
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
        description: "Synthetic refundable payment",
        merchant_order_reference_id: createReference("order"),
        return_url: testReturnUrl,
      }));
    await report.check(
      "Refund-reversal payment fields",
      {
        actual: select(payment, [
          "status",
          "amount",
          "amount_received",
          "currency",
          "connector",
        ]),
        expected: expected("refund-reversal-payment-fields", {
          status: "succeeded",
          amount: 6000,
          amount_received: 6000,
          currency: "USD",
          connector: connector.id,
        }),
      },
      ({ actual, expected }) => expect(actual).toMatchObject(expected),
    );

    const refund = await report.step("Create the reversible refund", () =>
      refunds.createRefund("reversible-refund", {
        payment_id: payment.payment_id,
        amount: 2000,
        reason: "REVERSIBLE TEST REFUND",
      }));
    await report.check(
      "Reversible refund fields",
      {
        actual: select(refund, ["status", "amount", "payment_id", "currency"]),
        expected: expected("reversible-refund-fields", {
          status: "succeeded",
          amount: 2000,
          payment_id: payment.payment_id,
          currency: "USD",
        }),
      },
      ({ actual, expected }) => expect(actual).toMatchObject(expected),
    );

    const reversed = await report.step("Reverse the successful refund", () =>
      refunds.reverseRefund("reverse-refund", refund.refund_id, {
        cancellation_reason: "Synthetic refund reversal",
      }));
    await report.check(
      "Refund-reversal response fields",
      {
        actual: select(reversed, [
          "refund_id",
          "payment_id",
          "status",
          "amount",
          "currency",
        ]),
        expected: expected("refund-reversal-fields", {
          refund_id: refund.refund_id,
          payment_id: payment.payment_id,
          status: "succeeded",
          amount: 2000,
          currency: "USD",
        }),
      },
      ({ actual, expected }) => expect(actual).toMatchObject(expected),
    );

    const recorded = await refunds.readRecordedRefundReversal(
      refund.refund_id,
      payment.payment_id,
    );
    await report.check(
      "Recorded refund-reversal outcome",
      {
        actual: recorded,
        expected: expected("refund-reversal-recorded-outcome", {
          status: "success",
          connectorRefundIdPresent: true,
          paymentIdMatches: true,
          reversalStatus: "success",
          reversalConnectorRefundIdPresent: true,
        }),
      },
      ({ actual, expected }) => expect(actual).toEqual(expected),
    );
  },
);

function select(response: Record<string, unknown>, fields: string[]) {
  return Object.fromEntries(fields.map((field) => [field, response[field]]));
}
