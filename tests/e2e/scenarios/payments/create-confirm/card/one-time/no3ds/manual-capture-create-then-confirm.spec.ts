import { expect } from "@playwright/test";
import { test } from "../../../../../../framework/fixtures.js";
import { scenarioFromFile } from "../../../../../catalog.js";
import {
  createAddress,
  createBrowserInfo,
  createCard,
  createCustomer,
  createOrderItem,
  createReference,
  testReturnUrl,
} from "../../../../../../test-data.js";

const scenario = scenarioFromFile(import.meta.url, {
  title: "One-time No3DS card — manual capture, create then confirm",
  description:
    "Create with confirm=false, confirm separately with capture_method=manual, verify authorization, then fully capture the payment.",
  verifies:
    "Check requires_confirmation followed by the authorized intermediate state, then verify the exact full-capture response and recorded UCS outcome. Also verify ownership, errors, amounts, checkout details and no mandate setup.",
  requirements: {
    connectorAccount: {
      paymentMethodsEnabled: [
        {
          payment_method: "card",
          payment_method_types: [
            {
              payment_method_type: "credit",
            },
          ],
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
    const metadata = { order_type: "one_time", fulfillment: "delivery" };
    const description = "Test order";
    const orderReference = createReference("order");
    const orderDetails = [createOrderItem({ quantity: 2, amount: 3000 })];
    const browserInfo = createBrowserInfo();
    const created = await report.step(
      "Create payment without confirmation",
      async () => {
        const response = await payments.createPayment("initial-payment", {
          profile_id: merchant.profileId,
          amount: 6000,
          currency: "USD",
          confirm: false,
          authentication_type: "no_three_ds",
          capture_method: "manual",
          payment_method: "card",
          payment_method_type: "credit",
          payment_method_data: { card: createCard(connector.card) },
          billing: {
            address: billingAddress,
            phone,
            email: customer.email,
          },
          shipping: { address: shippingAddress, phone, email: customer.email },
          browser_info: browserInfo,
          customer,
          description,
          metadata,
          merchant_order_reference_id: orderReference,
          order_details: orderDetails,
          return_url: testReturnUrl,
        });

        await report.check(
          "Created payment",
          {
            actual: {
              status: response.status,
              amount: response.amount,
              currency: response.currency,
              paymentMethod: response.payment_method,
            },
            expected: {
              status: "requires_confirmation",
              amount: 6000,
              currency: "USD",
              paymentMethod: "card",
            },
          },
          ({ actual, expected }) => expect(actual).toEqual(expected),
        );
        return response;
      },
    );

    const payment = await report.step(
      "Confirm payment using the server key",
      async () => {
        const response = await payments.confirmPayment(
          "confirm-payment",
          created.payment_id,
          {
            profile_id: merchant.profileId,
            browser_info: browserInfo,
            return_url: testReturnUrl,
          },
        );

        const expectedConfirmation = expected("confirmation-fields", {
          status: "requires_capture",
          error_code: null,
          error_message: null,
          error_details: null,
          issuer_error_code: null,
          issuer_error_message: null,
          merchant_id: merchant.merchantId,
          profile_id: merchant.profileId,
          connector: connector.id,
          merchant_connector_id: merchant.merchantConnectorId,
          capture_method: "manual",
          payment_method: "card",
          payment_method_type: "credit",
          authentication_type: "no_three_ds",
          amount: 6000,
          net_amount: 6000,
          amount_capturable: 6000,
          currency: "USD",
          amount_received: null,
          setup_future_usage: null,
          next_action: null,
          mandate_id: null,
          mandate_data: null,
          connector_mandate_id: null,
          attempt_count: 1,
          email: customer.email,
          customer_id: customer.id,
          name: customer.name,
          phone: customer.phone,
          description,
          metadata,
          merchant_order_reference_id: orderReference,
        });
        // Report only the selected fields, never the complete payment response.
        await report.check(
          "Confirmation fields",
          {
            actual: Object.fromEntries(
              Object.keys(expectedConfirmation).map((field) => [
                field,
                response[field],
              ]),
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
            expected: expected("customer-and-order-details", {
              billing: {
                address: billingAddress,
                phone,
                email: customer.email,
              },
              shipping: {
                address: shippingAddress,
                phone,
                email: customer.email,
              },
              customer,
              browser_info: browserInfo,
              order_details: orderDetails,
            }),
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );

        // Compare URL values after standard URL normalization.
        const actualReturnUrl = response.return_url;
        await report.check(
          "Return URL",
          {
            actual:
              typeof actualReturnUrl === "string"
                ? new URL(actualReturnUrl).href
                : null,
            expected: new URL(testReturnUrl).href,
          },
          ({ actual, expected }) => expect(actual).toBe(expected),
        );

        // Report presence and validity rather than exposing generated references.
        await report.check(
          "Generated payment references",
          {
            actual: {
              paymentIdPresent:
                typeof response.payment_id === "string" &&
                response.payment_id.trim().length > 0,
              connectorTransactionIdPresent:
                typeof response.connector_transaction_id === "string" &&
                response.connector_transaction_id.trim().length > 0,
            },
            expected: {
              paymentIdPresent: true,
              connectorTransactionIdPresent: true,
            },
          },
          ({ actual, expected }) => expect(actual).toEqual(expected),
        );
        return response;
      },
    );

    // Read stored facts without triggering connector synchronization.
    await report.step("Verify recorded authorization", async () => {
      const recorded = await payments.readRecordedPayment(payment.payment_id);
      await report.check(
        "Recorded payment outcome",
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

    await report.step("Capture the full authorized amount", async () => {
      const response = await payments.capturePayment(
        "full-capture",
        payment.payment_id,
        {
          amount_to_capture: 6000,
          statement_descriptor_prefix: "SYNTHETIC",
          statement_descriptor_suffix: "E2E",
        },
      );
      const expectedCapture = expected("capture-fields", {
        status: "succeeded",
        amount: 6000,
        currency: "USD",
        amount_capturable: 0,
        amount_received: 6000,
        error_code: null,
        error_message: null,
        connector: connector.id,
        capture_method: "manual",
      });
      await report.check(
        "Capture response",
        {
          actual: Object.fromEntries(
            Object.keys(expectedCapture).map((field) => [
              field,
              response[field],
            ]),
          ),
          expected: expectedCapture,
        },
        ({ actual, expected }) => expect(actual).toMatchObject(expected),
      );
      await report.check(
        "Capture identity",
        {
          actual: response.payment_id === payment.payment_id,
          expected: true,
        },
        ({ actual, expected }) => expect(actual).toBe(expected),
      );
    });

    await report.step("Verify recorded capture response", async () => {
      const recorded = await payments.readRecordedPayment(payment.payment_id);
      await report.check(
        "Captured attempt",
        {
          actual: recorded,
          expected: expected("capture-recorded-outcome", {
            gateway: "unified_connector_service",
            attemptStatus: "charged",
            connectorTransactionIdPresent: true,
            customerRecordExists: true,
          }),
        },
        ({ actual, expected }) => expect(actual).toEqual(expected),
      );
    });
  },
);
