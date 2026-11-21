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
  title: "Fully void an authorized No3DS card payment",
  description:
    "Authorize a richly populated manual-capture payment, verify the held amount, then cancel the authorization before capture.",
  verifies:
    "Verify the authorized intermediate state, exact cancellation response, released capturable amount, voided UCS attempt and preserved payment ownership.",
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
  async ({ connector, merchant, payments, report, expected }) => {
    const billingAddress = createAddress();
    const shippingAddress = createAddress({ line1: "2" });
    const customer = createCustomer();
    const phone = {
      number: customer.phone,
      country_code: customer.phone_country_code,
    };
    const metadata = { order_type: "one_time", operation: "void" };
    const description = "Synthetic order for void";
    const orderReference = createReference("order");
    const orderDetails = [createOrderItem({ quantity: 2, amount: 3000 })];
    const browserInfo = createBrowserInfo();

    const payment = await report.step(
      "Authorize payment for later void",
      async () => {
        const response = await payments.createPayment("initial-payment", {
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
          browser_info: browserInfo,
          customer,
          description,
          metadata,
          merchant_order_reference_id: orderReference,
          order_details: orderDetails,
          return_url: testReturnUrl,
        });
        const expectedAuthorization = expected("authorization-fields", {
          status: "requires_capture",
          amount: 6000,
          currency: "USD",
          amount_capturable: 6000,
          amount_received: null,
          capture_method: "manual",
          connector: connector.id,
          merchant_id: merchant.merchantId,
          profile_id: merchant.profileId,
          merchant_connector_id: merchant.merchantConnectorId,
          error_code: null,
          error_message: null,
        });
        await report.check(
          "Authorization response",
          {
            actual: Object.fromEntries(
              Object.keys(expectedAuthorization).map((field) => [
                field,
                response[field],
              ]),
            ),
            expected: expectedAuthorization,
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );
        return response;
      },
    );

    await report.step("Verify recorded authorization", async () => {
      const recorded = await payments.readRecordedPayment(payment.payment_id);
      await report.check(
        "Authorized attempt",
        {
          actual: recorded,
          expected: {
            gateway: "unified_connector_service",
            attemptStatus: "authorized",
            connectorTransactionIdPresent: true,
            customerRecordExists: true,
          },
        },
        ({ actual, expected }) => expect(actual).toEqual(expected),
      );
    });

    await report.step("Cancel the full authorization", async () => {
      const response = await payments.cancelPayment(
        "full-void",
        payment.payment_id,
        { cancellation_reason: "requested_by_customer" },
      );
      const expectedCancellation = expected("cancellation-fields", {
        status: "cancelled",
        amount: 6000,
        currency: "USD",
        amount_capturable: 0,
        amount_received: null,
        capture_method: "manual",
        connector: connector.id,
        error_code: null,
        error_message: null,
      });
      await report.check(
        "Cancellation response",
        {
          actual: Object.fromEntries(
            Object.keys(expectedCancellation).map((field) => [
              field,
              response[field],
            ]),
          ),
          expected: expectedCancellation,
        },
        ({ actual, expected }) => expect(actual).toMatchObject(expected),
      );
      await report.check(
        "Cancellation identity",
        {
          actual: response.payment_id === payment.payment_id,
          expected: true,
        },
        ({ actual, expected }) => expect(actual).toBe(expected),
      );
    });

    await report.step("Verify recorded void", async () => {
      const recorded = await payments.readRecordedPayment(payment.payment_id);
      await report.check(
        "Voided attempt",
        {
          actual: recorded,
          expected: expected("cancellation-recorded-outcome", {
            gateway: "unified_connector_service",
            attemptStatus: "voided",
            connectorTransactionIdPresent: true,
            customerRecordExists: true,
          }),
        },
        ({ actual, expected }) => expect(actual).toEqual(expected),
      );
    });
  },
);
