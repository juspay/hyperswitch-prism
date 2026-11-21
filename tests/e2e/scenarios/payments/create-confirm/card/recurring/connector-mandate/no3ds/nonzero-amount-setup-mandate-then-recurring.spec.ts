import { expect } from "@playwright/test";
import { test } from "../../../../../../../framework/fixtures.js";
import { scenarioFromFile } from "../../../../../../catalog.js";
import {
  createAddress,
  createBrowserInfo,
  createCard,
  createCustomer,
  createCustomerAcceptance,
  createOrderItem,
  createReference,
  testReturnUrl,
} from "../../../../../../../test-data.js";

const scenario = scenarioFromFile(import.meta.url, {
  title: "No3DS card mandate — nonzero setup then recurring charge",
  description:
    "Create and capture a customer-present No3DS card payment that establishes a reusable off-session mandate, then charge that mandate in a second automatic-capture payment without resending card data.",
  verifies:
    "Verify the setup payment, reusable mandate references and recorded active mandate; then verify the off-session recurring charge succeeds for the same customer and mandate with a distinct payment and processor reference.",
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
    const browserInfo = createBrowserInfo();
    const customerAcceptance = createCustomerAcceptance();
    const setupMetadata = { payment_purpose: "mandate_setup" };
    const setupDescription = "Synthetic subscription setup";
    const setupOrderReference = createReference("order");
    const setupOrderDetails = [createOrderItem({ quantity: 2, amount: 3000 })];

    const setupPayment = await report.step(
      "Create, confirm and capture the mandate setup payment",
      async () => {
        const response = await payments.createPayment("mandate-setup", {
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
            email: customer.email,
          },
          shipping: {
            address: shippingAddress,
            phone,
            email: customer.email,
          },
          browser_info: browserInfo,
          customer,
          customer_acceptance: customerAcceptance,
          setup_future_usage: "off_session",
          mandate_data: {
            customer_acceptance: customerAcceptance,
            mandate_type: { multi_use: null },
          },
          description: setupDescription,
          metadata: setupMetadata,
          merchant_order_reference_id: setupOrderReference,
          order_details: setupOrderDetails,
          return_url: testReturnUrl,
        });

        const expectedSetup = expected("mandate-setup-fields", {
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
          setup_future_usage: "off_session",
          next_action: null,
          mandate_data: {
            customer_acceptance: customerAcceptance,
            mandate_type: { multi_use: null },
          },
          connector_mandate_id: null,
          attempt_count: 1,
          email: customer.email,
          customer_id: customer.id,
          name: customer.name,
          phone: customer.phone,
          description: setupDescription,
          metadata: setupMetadata,
          merchant_order_reference_id: setupOrderReference,
        });
        await report.check(
          "Mandate setup payment fields",
          {
            actual: Object.fromEntries(
              Object.keys(expectedSetup).map((field) => [
                field,
                response[field],
              ]),
            ),
            expected: expectedSetup,
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );
        await report.check(
          "Mandate setup customer and order details",
          {
            actual: {
              billing: response.billing,
              shipping: response.shipping,
              customer: response.customer,
              browser_info: response.browser_info,
              order_details: response.order_details,
            },
            expected: expected("mandate-setup-customer-and-order-details", {
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
              order_details: setupOrderDetails,
            }),
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );
        await report.check(
          "Generated setup and mandate references",
          {
            actual: {
              paymentIdPresent: nonempty(response.payment_id),
              connectorTransactionIdPresent: nonempty(
                response.connector_transaction_id,
              ),
              mandateIdPresent: nonempty(response.mandate_id),
            },
            expected: {
              paymentIdPresent: true,
              connectorTransactionIdPresent: true,
              mandateIdPresent: true,
            },
          },
          ({ actual, expected }) => expect(actual).toEqual(expected),
        );
        return response;
      },
    );

    const mandateId = requiredString(setupPayment.mandate_id, "mandate_id");
    await report.step(
      "Verify the reusable mandate recorded by HS",
      async () => {
        const recorded = await payments.readRecordedMandate(mandateId);
        await report.check(
          "Recorded mandate",
          {
            actual: recorded,
            expected: expected("recorded-mandate", {
              status: "active",
              type: "multi_use",
              connector: connector.id,
              connectorMandateIdPresent: true,
              paymentMethodIdPresent: true,
            }),
          },
          ({ actual, expected }) => expect(actual).toEqual(expected),
        );
      },
    );

    const recurringMetadata = { payment_purpose: "recurring_charge" };
    const recurringDescription = "Synthetic recurring charge";
    const recurringOrderReference = createReference("order");
    const recurringPayment = await report.step(
      "Charge the mandate without resending card data",
      async () => {
        const response = await payments.createPayment("recurring-payment", {
          profile_id: merchant.profileId,
          amount: 2500,
          currency: "USD",
          confirm: true,
          authentication_type: "no_three_ds",
          capture_method: "automatic",
          customer_id: customer.id,
          mandate_id: mandateId,
          off_session: true,
          description: recurringDescription,
          metadata: recurringMetadata,
          merchant_order_reference_id: recurringOrderReference,
          return_url: testReturnUrl,
        });

        const expectedRecurring = expected("recurring-payment-fields", {
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
          amount: 2500,
          net_amount: 2500,
          amount_capturable: 0,
          currency: "USD",
          amount_received: 2500,
          setup_future_usage: null,
          next_action: null,
          mandate_id: mandateId,
          attempt_count: 1,
          customer_id: customer.id,
          description: recurringDescription,
          metadata: recurringMetadata,
          merchant_order_reference_id: recurringOrderReference,
        });
        await report.check(
          "Recurring payment fields",
          {
            actual: Object.fromEntries(
              Object.keys(expectedRecurring).map((field) => [
                field,
                response[field],
              ]),
            ),
            expected: expectedRecurring,
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );
        await report.check(
          "Recurring payment references",
          {
            actual: {
              paymentIdPresent: nonempty(response.payment_id),
              newPaymentCreated:
                response.payment_id !== setupPayment.payment_id,
              connectorTransactionIdPresent: nonempty(
                response.connector_transaction_id,
              ),
              mandateReused: response.mandate_id === mandateId,
            },
            expected: {
              paymentIdPresent: true,
              newPaymentCreated: true,
              connectorTransactionIdPresent: true,
              mandateReused: true,
            },
          },
          ({ actual, expected }) => expect(actual).toEqual(expected),
        );
        return response;
      },
    );

    await report.step("Verify the recorded recurring charge", async () => {
      const recorded = await payments.readRecordedPayment(
        recurringPayment.payment_id,
      );
      await report.check(
        "Recorded recurring payment",
        {
          actual: recorded,
          expected: expected("recorded-recurring-payment", {
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

function nonempty(value: unknown): boolean {
  return typeof value === "string" && value.trim().length > 0;
}

function requiredString(value: unknown, field: string): string {
  if (!nonempty(value)) throw new Error(`Response must contain ${field}`);
  return value as string;
}
