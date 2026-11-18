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
  title: "Force-sync a successful No3DS card payment",
  description:
    "Create an automatically captured payment, then retrieve it with force_sync=true so HS performs payment sync through UCS.",
  verifies:
    "Verify the synchronized response keeps the exact payment identity, ownership, successful amount state and charged UCS attempt.",
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
    const metadata = { order_type: "one_time", operation: "payment_sync" };
    const description = "Synthetic order for payment sync";
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
          "Payment response",
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

    await report.step("Force-sync the payment", async () => {
      const response = await payments.retrievePayment(payment.payment_id, {
        force_sync: true,
      });
      const expectedSync = expected("payment-sync-fields", {
        status: "succeeded",
        amount: 6000,
        currency: "USD",
        amount_capturable: 0,
        amount_received: 6000,
        capture_method: "automatic",
        payment_method: "card",
        payment_method_type: "credit",
        authentication_type: "no_three_ds",
        connector: connector.id,
        merchant_id: merchant.merchantId,
        profile_id: merchant.profileId,
        merchant_connector_id: merchant.merchantConnectorId,
        customer_id: customer.id,
        description,
        metadata,
        merchant_order_reference_id: orderReference,
        error_code: null,
        error_message: null,
      });
      await report.check(
        "Synchronized payment response",
        {
          actual: Object.fromEntries(
            Object.keys(expectedSync).map((field) => [field, response[field]]),
          ),
          expected: expectedSync,
        },
        ({ actual, expected }) => expect(actual).toMatchObject(expected),
      );
      await report.check(
        "Synchronized payment identity",
        {
          actual: response.payment_id === payment.payment_id,
          expected: true,
        },
        ({ actual, expected }) => expect(actual).toBe(expected),
      );
    });

    await report.step("Verify recorded synchronized payment", async () => {
      const recorded = await payments.readRecordedPayment(payment.payment_id);
      await report.check(
        "Recorded synchronized outcome",
        {
          actual: recorded,
          expected: {
            gateway: "unified_connector_service",
            attemptStatus: "charged",
            connectorTransactionIdPresent: true,
            customerRecordExists: true,
          },
        },
        ({ actual, expected }) => expect(actual).toEqual(expected),
      );
    });
  },
);
