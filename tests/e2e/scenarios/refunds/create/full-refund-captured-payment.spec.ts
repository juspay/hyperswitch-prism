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
  title: "Submit a full refund for a captured No3DS card payment",
  description:
    "Create an automatically captured payment, submit a full refund, and verify the accepted asynchronous refund recorded by HS.",
  verifies:
    "Verify payment capture, refund identity and amount, exact submission status, processor refund reference and the recorded refund-to-payment relationship.",
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
    const billingAddress = createAddress();
    const shippingAddress = createAddress({ line1: "2" });
    const customer = createCustomer();
    const phone = {
      number: customer.phone,
      country_code: customer.phone_country_code,
    };
    const metadata = { order_type: "one_time", operation: "full_refund" };
    const description = "Synthetic order for full refund";
    const orderReference = createReference("order");
    const orderDetails = [createOrderItem({ quantity: 2, amount: 3000 })];
    const browserInfo = createBrowserInfo();

    const payment = await report.step(
      "Create an automatically captured payment",
      async () => {
        const response = await payments.createPayment("initial-payment", {
          profile_id: merchant.profileId,
          amount: 6000,
          currency: "USD",
          confirm: true,
          authentication_type: "no_three_ds",
          capture_method: "automatic",
          payment_method: "card",
          payment_method_type: "credit",
          payment_method_data: { card: createCard(connector.card) },
          billing: { address: billingAddress, phone, email: customer.email },
          shipping: { address: shippingAddress, phone, email: customer.email },
          browser_info: browserInfo,
          customer,
          description,
          metadata,
          merchant_order_reference_id: orderReference,
          order_details: orderDetails,
          return_url: testReturnUrl,
        });
        const expectedPayment = expected("payment-fields", {
          status: "succeeded",
          amount: 6000,
          currency: "USD",
          amount_capturable: 0,
          amount_received: 6000,
          capture_method: "automatic",
          connector: connector.id,
          merchant_id: merchant.merchantId,
          profile_id: merchant.profileId,
          merchant_connector_id: merchant.merchantConnectorId,
          error_code: null,
          error_message: null,
        });
        await report.check(
          "Captured payment response",
          {
            actual: Object.fromEntries(
              Object.keys(expectedPayment).map((field) => [
                field,
                response[field],
              ]),
            ),
            expected: expectedPayment,
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );
        return response;
      },
    );

    const refund = await report.step("Create the full refund", async () => {
      const response = await refunds.createRefund("full-refund", {
        payment_id: payment.payment_id,
        amount: 6000,
        reason: "CUSTOMER REQUEST",
      });
      const expectedRefund = expected("refund-submission-fields", {
        status: "succeeded",
        amount: 6000,
        currency: "USD",
        payment_id: payment.payment_id,
        error_code: null,
        error_message: null,
      });
      await report.check(
        "Refund submission response",
        {
          actual: Object.fromEntries(
            Object.keys(expectedRefund).map((field) => [
              field,
              response[field],
            ]),
          ),
          expected: expectedRefund,
        },
        ({ actual, expected }) => expect(actual).toMatchObject(expected),
      );
      await report.check(
        "Generated refund reference",
        {
          actual: response.refund_id.trim().length > 0,
          expected: true,
        },
        ({ actual, expected }) => expect(actual).toBe(expected),
      );
      return response;
    });

    await report.step("Verify recorded refund", async () => {
      const recorded = await refunds.readRecordedRefund(
        refund.refund_id,
        payment.payment_id,
      );
      await report.check(
        "Recorded refund outcome",
        {
          actual: recorded,
          expected: expected("refund-recorded-outcome", {
            status: "success",
            connectorRefundIdPresent: true,
            paymentIdMatches: true,
          }),
        },
        ({ actual, expected }) => expect(actual).toEqual(expected),
      );
    });
  },
);
