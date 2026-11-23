import { expect } from "@playwright/test";
import { test } from "../../../../../../../framework/fixtures.js";
import { scenarioFromFile } from "../../../../../../catalog.js";
import {
  createAddress,
  createBrowserInfo,
  createCard,
  createCustomer,
  createCustomerAcceptance,
  createReference,
  testReturnUrl,
} from "../../../../../../../test-data.js";

const scenario = scenarioFromFile(import.meta.url, {
  title: "No3DS card mandate — zero-amount setup then recurring charge",
  description:
    "Create a zero-value customer-present card mandate setup with the selected capture mode, then make an off-session recurring payment using the resulting connector mandate and no card data.",
  verifies:
    "For both automatic and manual capture settings, verify the zero-value setup response, active recorded connector mandate, and successful recurring charge for the same customer.",
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

for (const captureMethod of ["automatic", "manual"] as const) {
  test(`${scenario.title} — ${captureMethod} capture setting`, async ({
    connector,
    merchant,
    payments,
    report,
    expected,
  }) => {
    await runZeroAmountSetupAndRecurring({
      captureMethod,
      connector,
      merchant,
      payments,
      report,
      expected,
    });
  });
}

async function runZeroAmountSetupAndRecurring({
  captureMethod,
  connector,
  merchant,
  payments,
  report,
  expected,
}: {
  captureMethod: "automatic" | "manual";
  connector: import("../../../../../../../framework/types.js").ConnectorDefinition;
  merchant: { merchantId: string; profileId: string; merchantConnectorId: string };
  payments: import("../../../../../../../framework/payment-operations.js").PaymentOperations;
  report: import("../../../../../../../framework/fixtures.js").ScenarioFixtures["report"];
  expected: import("../../../../../../../framework/fixtures.js").ScenarioFixtures["expected"];
}): Promise<void> {
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
  const setupDescription = "Synthetic zero-value mandate setup";
  const setupOrderReference = createReference("order");

  const setupPayment = await report.step(
    `Create and confirm zero-amount setup (${captureMethod} capture setting)`,
    async () => {
      const response = await payments.createPayment("mandate-setup", {
        profile_id: merchant.profileId,
        amount: 0,
        currency: "USD",
        confirm: true,
        authentication_type: "no_three_ds",
        capture_method: captureMethod,
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
        return_url: testReturnUrl,
      });
      const expectedSetup = expected(`zero-setup-fields-${captureMethod}`, {
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
        capture_method: captureMethod,
        payment_method: "card",
        payment_method_type: "credit",
        authentication_type: "no_three_ds",
        amount: 0,
        net_amount: 0,
        amount_capturable: 0,
        currency: "USD",
        amount_received: 0,
        setup_future_usage: "off_session",
        next_action: null,
        mandate_data: {
          customer_acceptance: customerAcceptance,
          mandate_type: { multi_use: null },
        },
        mandate_id: null,
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
        `Zero-value setup response (${captureMethod})`,
        {
          actual: Object.fromEntries(
            Object.keys(expectedSetup).map((field) => [field, response[field]]),
          ),
          expected: expectedSetup,
        },
        ({ actual, expected }) => expect(actual).toMatchObject(expected),
      );
      await report.check(
        "Zero-value setup customer and address fields",
        {
          actual: {
            billing: response.billing,
            shipping: response.shipping,
            customer: response.customer,
            browser_info: response.browser_info,
          },
          expected: expected(`zero-setup-customer-fields-${captureMethod}`, {
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
          }),
        },
        ({ actual, expected }) => expect(actual).toMatchObject(expected),
      );
      await report.check(
        "Setup payment and mandate references",
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
  await report.step("Verify the recorded zero-value setup payment", async () => {
    const recorded = await payments.readRecordedPayment(setupPayment.payment_id);
    await report.check(
      "Recorded zero-value setup outcome",
      {
        actual: recorded,
        expected: expected(`recorded-zero-setup-${captureMethod}`, {
          gateway: "unified_connector_service",
          attemptStatus: "charged",
          connectorTransactionIdPresent: true,
          customerRecordExists: true,
        }),
      },
      ({ actual, expected }) => expect(actual).toEqual(expected),
    );
  });

  await report.step("Verify the active connector mandate", async () => {
    const recorded = await payments.readRecordedMandate(mandateId);
    await report.check(
      "Recorded zero-value setup mandate",
      {
        actual: recorded,
        expected: expected(`recorded-zero-mandate-${captureMethod}`, {
          status: "active",
          type: "multi_use",
          connector: connector.id,
          connectorMandateIdPresent: true,
          paymentMethodIdPresent: true,
        }),
      },
      ({ actual, expected }) => expect(actual).toEqual(expected),
    );
  });

  const recurringPayment = await report.step(
    "Charge the mandate without resending card data",
    async () => {
      const description = "Synthetic recurring charge";
      const metadata = { payment_purpose: "recurring_charge" };
      const orderReference = createReference("order");
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
        description,
        metadata,
        merchant_order_reference_id: orderReference,
        return_url: testReturnUrl,
      });
      const expectedRecurring = expected(
        `recurring-payment-fields-${captureMethod}`,
        {
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
          description,
          metadata,
          merchant_order_reference_id: orderReference,
        },
      );
      await report.check(
        "Recurring payment response fields",
        {
          actual: Object.fromEntries(
            Object.keys(expectedRecurring).map((field) => [field, response[field]]),
          ),
          expected: expectedRecurring,
        },
        ({ actual, expected }) => expect(actual).toMatchObject(expected),
      );
      await report.check(
        "Recurring payment reuses the mandate",
        {
          actual: {
            paymentIdPresent: nonempty(response.payment_id),
            isNewPayment: response.payment_id !== setupPayment.payment_id,
            connectorTransactionIdPresent: nonempty(
              response.connector_transaction_id,
            ),
            mandateReused: response.mandate_id === mandateId,
          },
          expected: {
            paymentIdPresent: true,
            isNewPayment: true,
            connectorTransactionIdPresent: true,
            mandateReused: true,
          },
        },
        ({ actual, expected }) => expect(actual).toEqual(expected),
      );
      return response;
    },
  );

  await report.step("Verify the recorded recurring payment", async () => {
    const recorded = await payments.readRecordedPayment(
      recurringPayment.payment_id,
    );
    await report.check(
      "Recorded recurring outcome",
      {
        actual: recorded,
        expected: expected(`recorded-recurring-payment-${captureMethod}`, {
          gateway: "unified_connector_service",
          attemptStatus: "charged",
          connectorTransactionIdPresent: true,
          customerRecordExists: true,
        }),
      },
      ({ actual, expected }) => expect(actual).toEqual(expected),
    );
  });
}

function nonempty(value: unknown): boolean {
  return typeof value === "string" && value.trim().length > 0;
}

function requiredString(value: unknown, field: string): string {
  if (!nonempty(value)) throw new Error(`Response must contain ${field}`);
  return value as string;
}
