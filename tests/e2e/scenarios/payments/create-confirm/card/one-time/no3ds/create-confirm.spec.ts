import { randomUUID } from "node:crypto";
import { expect } from "@playwright/test";
import { test } from "../../../../../../framework/fixtures.js";
import { connectors } from "../../../../../../connectors/index.js";
import {
  createAddress,
  createBrowserInfo,
  createCard,
  createCustomer,
  testReturnUrl,
} from "../../../../../../test-data.js";

const scenario = {
  id: "no3ds-create-confirm",
  title: "One-time No3DS card payment — create and confirm with customer and order details",
  description:
    "Create and confirm a one-time No3DS card payment with a new customer, billing, shipping and order details in one API request. Verify succeeded status, the full amount received and the recorded charged attempt through UCS.",
  verifies:
    "Verify customer creation and association, request details preserved in the response, full amount received with nothing left capturable, merchant and profile ownership, one attempt, no errors or mandate setup, and a recorded UCS gateway and connector transaction ID.",
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
    test(scenario.title, async ({ connector, merchant, payments, report }) => {
      const billingAddress = createAddress();
      const shippingAddress = createAddress({ line1: "2" });
      const customer = createCustomer();
      const email = customer.email;
      const returnUrl = testReturnUrl;
      const phone = {
        number: customer.phone,
        country_code: customer.phone_country_code,
      };
      const metadata = { order_type: "one_time", fulfillment: "delivery" };
      const description = "Test order";
      const orderReference = `order_${randomUUID().replaceAll("-", "")}`;
      const orderDetails = [
        {
          product_name: "Test product",
          product_id: "test-product",
          quantity: 2,
          amount: 3000,
          requires_shipping: true,
        },
      ];
      const browserInfo = createBrowserInfo();
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
          payment_method_data: { card: createCard(connector.card) },
          billing: {
            address: billingAddress,
            phone,
            email,
          },
          shipping: { address: shippingAddress, phone, email },
          browser_info: browserInfo,
          customer,
          description,
          metadata,
          merchant_order_reference_id: orderReference,
          order_details: orderDetails,
          return_url: returnUrl,
        });

        const expectedConfirmation = {
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
          customer_id: customer.id,
          name: customer.name,
          phone: customer.phone,
          description,
          metadata,
          merchant_order_reference_id: orderReference,
        };
        // Report only the selected fields, never the complete payment response.
        await report.check(
          "Confirmation fields",
          {
            actual: Object.fromEntries(
              Object.keys(expectedConfirmation).map((field) => [field, response[field]]),
            ),
            expected: expectedConfirmation,
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );
        await report.check(
          "Customer, addresses, browser and order details",
          {
            actual: {
              billing: response.billing,
              shipping: response.shipping,
              customer: response.customer,
              browser_info: response.browser_info,
              order_details: response.order_details,
            },
            expected: {
              billing: { address: billingAddress, phone, email },
              shipping: { address: shippingAddress, phone, email },
              customer,
              browser_info: browserInfo,
              order_details: orderDetails,
            },
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );

        // Compare URL values after standard URL normalization.
        const actualReturnUrl = response.return_url;
        await report.check(
          "Return URL",
          {
            actual: typeof actualReturnUrl === "string"
              ? new URL(actualReturnUrl).href
              : null,
            expected: new URL(returnUrl).href,
          },
          ({ actual, expected }) => expect(actual).toBe(expected),
        );

        // Report presence and validity rather than exposing generated references.
        await report.check(
          "Generated payment references",
          {
            actual: {
              paymentIdPresent: typeof response.payment_id === "string"
                && response.payment_id.trim().length > 0,
              connectorTransactionIdPresent:
                typeof response.connector_transaction_id === "string"
                && response.connector_transaction_id.trim().length > 0,
            },
            expected: { paymentIdPresent: true, connectorTransactionIdPresent: true },
          },
          ({ actual, expected }) => expect(actual).toEqual(expected),
        );
        await report.check(
          "Payment timestamps",
          {
            actual: {
              createdValid: typeof response.created === "string"
                && Number.isFinite(Date.parse(response.created)),
              modifiedValid: typeof response.modified_at === "string"
                && Number.isFinite(Date.parse(response.modified_at)),
            },
            expected: { createdValid: true, modifiedValid: true },
          },
          ({ actual, expected }) => expect(actual).toEqual(expected),
        );
        return response;
      });

      // 2. Read recorded facts for this payment and its active attempt.
      // This does not trigger or prove a fresh processor PSync call.
      await report.step("Verify recorded payment outcome", async () => {
        const recorded = await payments.readRecordedPayment(payment.payment_id);
        await report.check(
          "Recorded payment outcome",
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
    });
  });
}
