import { expect } from "@playwright/test";
import { test } from "../../../../../../framework/fixtures.js";
import { scenarioFromFile } from "../../../../../catalog.js";
import {
  createAddress,
  createBrowserInfo,
  createCard,
  createCustomer,
  createCustomerAcceptance,
  createReference,
  testReturnUrl,
} from "../../../../../../test-data.js";

const scenario = scenarioFromFile(import.meta.url, {
  title: "Repeat a customer-initiated payment with a saved card",
  description:
    "Complete an on-session payment that saves the card, obtain its customer-scoped payment token, then use that token for a second customer-initiated payment without resending PAN data.",
  verifies:
    "Verify both payments succeed, HS persists and returns a saved payment method, the repeat request uses its token, and both UCS attempts retain processor references.",
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
    const customer = createCustomer();
    const billing = { address: createAddress(), email: customer.email };
    const browserInfo = createBrowserInfo();

    const initial = await report.step("Pay and save the card", async () => {
      const response = await payments.createPayment("save-card-payment", {
        profile_id: merchant.profileId,
        amount: 6000,
        currency: "USD",
        confirm: true,
        authentication_type: "no_three_ds",
        capture_method: "automatic",
        payment_method: "card",
        payment_method_type: "credit",
        payment_method_data: { card: createCard(connector.card) },
        setup_future_usage: "on_session",
        customer_acceptance: createCustomerAcceptance(),
        customer,
        billing,
        browser_info: browserInfo,
        description: "Synthetic saved-card initial payment",
        metadata: { operation: "save_card" },
        merchant_order_reference_id: createReference("order"),
        return_url: testReturnUrl,
      });
      await report.check(
        "Saved-card initial payment fields",
        {
          actual: select(response, [
            "status",
            "amount",
            "currency",
            "amount_received",
            "setup_future_usage",
            "customer_id",
            "connector",
            "error_code",
            "error_message",
          ]),
          expected: expected("saved-card-initial-fields", {
            status: "succeeded",
            amount: 6000,
            currency: "USD",
            amount_received: 6000,
            setup_future_usage: "on_session",
            customer_id: customer.id,
            connector: connector.id,
            error_code: null,
            error_message: null,
          }),
        },
        ({ actual, expected }) => expect(actual).toMatchObject(expected),
      );
      return response;
    });

    const paymentToken = await report.step(
      "Read the customer saved payment method",
      async () => {
        const response = await payments.listCustomerPaymentMethods(customer.id);
        const methods = response.customer_payment_methods;
        const first = Array.isArray(methods) ? methods[0] : undefined;
        const token = objectStringField(first, "payment_token");
        await report.check(
          "Saved payment method is available",
          {
            actual: {
              methodCountPositive: Array.isArray(methods) && methods.length > 0,
              paymentTokenPresent: token.length > 0,
              paymentMethodIdPresent:
                objectStringField(first, "payment_method_id").length > 0,
            },
            expected: {
              methodCountPositive: true,
              paymentTokenPresent: true,
              paymentMethodIdPresent: true,
            },
          },
          ({ actual, expected }) => expect(actual).toEqual(expected),
        );
        return token;
      },
    );

    const repeatIntent = await report.step(
      "Create the repeat payment intent",
      () =>
        payments.createPayment("repeat-payment-intent", {
          profile_id: merchant.profileId,
          amount: 4000,
          currency: "USD",
          confirm: false,
          authentication_type: "no_three_ds",
          capture_method: "automatic",
          payment_method: "card",
          payment_method_type: "credit",
          customer_id: customer.id,
          description: "Synthetic saved-card repeat payment",
          merchant_order_reference_id: createReference("order"),
          return_url: testReturnUrl,
        }),
    );

    const repeated = await report.step(
      "Confirm with the saved payment token",
      () =>
        payments.confirmPayment(
          "repeat-payment-with-saved-card",
          repeatIntent.payment_id,
          {
            payment_method: "card",
            payment_token: paymentToken,
            billing,
            browser_info: browserInfo,
            return_url: testReturnUrl,
          },
        ),
    );
    await report.check(
      "Saved-card repeat payment fields",
      {
        actual: select(repeated, [
          "payment_id",
          "status",
          "amount",
          "currency",
          "amount_received",
          "customer_id",
          "connector",
          "error_code",
          "error_message",
        ]),
        expected: expected("saved-card-repeat-fields", {
          payment_id: repeatIntent.payment_id,
          status: "succeeded",
          amount: 4000,
          currency: "USD",
          amount_received: 4000,
          customer_id: customer.id,
          connector: connector.id,
          error_code: null,
          error_message: null,
        }),
      },
      ({ actual, expected }) => expect(actual).toMatchObject(expected),
    );

    for (const payment of [initial, repeated]) {
      const recorded = await payments.readRecordedPayment(payment.payment_id);
      await report.check(
        `Recorded saved-card payment ${payment.payment_id === initial.payment_id ? "initial" : "repeat"}`,
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
    }
  },
);

function select(response: Record<string, unknown>, fields: string[]) {
  return Object.fromEntries(fields.map((field) => [field, response[field]]));
}

function objectStringField(value: unknown, field: string): string {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return "";
  }
  const item = (value as Record<string, unknown>)[field];
  return typeof item === "string" ? item : "";
}
