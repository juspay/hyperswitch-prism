import { expect } from "@playwright/test";
import { test } from "../../../framework/fixtures.js";
import { scenarioFromFile } from "../../catalog.js";
import {
  createAddress,
  createBrowserInfo,
  createCard,
  createCustomer,
  createCustomerAcceptance,
  createReference,
  testReturnUrl,
} from "../../../test-data.js";

const scenario = scenarioFromFile(import.meta.url, {
  title: "Increase an authorized card-payment amount",
  description:
    "Authorize 6000 minor units with incremental authorization requested, increase the authorization to 8000, then capture the complete increased amount.",
  verifies:
    "Verify the original authorization, successful incremental-authorization record, increased capturable amount, terminal capture and recorded UCS state.",
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
    const billingAddress = createAddress();

    const payment = await report.step(
      "Authorize with incremental authorization enabled",
      async () => {
        const response = await payments.createPayment(
          "incremental-authorization-payment",
          {
            profile_id: merchant.profileId,
            amount: 6000,
            currency: "USD",
            confirm: true,
            authentication_type: "no_three_ds",
            capture_method: "manual",
            payment_method: "card",
            payment_method_type: "credit",
            payment_method_data: { card: createCard(connector.card) },
            request_incremental_authorization: true,
            setup_future_usage: "off_session",
            customer_acceptance: createCustomerAcceptance(),
            billing: { address: billingAddress, email: customer.email },
            browser_info: createBrowserInfo(),
            customer,
            description: "Synthetic incremental authorization",
            metadata: { operation: "incremental_authorization" },
            merchant_order_reference_id: createReference("order"),
            return_url: testReturnUrl,
          },
        );
        await report.check(
          "Initial incremental authorization fields",
          {
            actual: select(response, [
              "status",
              "amount",
              "currency",
              "amount_capturable",
              "amount_received",
              "capture_method",
              "incremental_authorization_allowed",
              "connector",
              "error_code",
              "error_message",
            ]),
            expected: expected("incremental-authorization-initial-fields", {
              status: "requires_capture",
              amount: 6000,
              currency: "USD",
              amount_capturable: 6000,
              amount_received: null,
              capture_method: "manual",
              incremental_authorization_allowed: true,
              connector: connector.id,
              error_code: null,
              error_message: null,
            }),
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );
        return response;
      },
    );

    await report.step("Increase the authorized amount", async () => {
      const response = await payments.incrementalAuthorization(
        "increase-authorization",
        payment.payment_id,
        { amount: 8000, reason: "Synthetic order amount increased" },
      );
      const authorizations = response.incremental_authorizations;
      await report.check(
        "Incremental authorization fields",
        {
          actual: {
            ...select(response, [
              "payment_id",
              "status",
              "amount",
              "amount_capturable",
              "amount_received",
              "incremental_authorization_allowed",
            ]),
            authorization_count: response.authorization_count,
            latest_authorization:
              Array.isArray(authorizations) && authorizations.length > 0
                ? authorizations.at(-1)
                : null,
          },
          expected: expected("incremental-authorization-fields", {
            payment_id: payment.payment_id,
            status: "requires_capture",
            amount: 8000,
            amount_capturable: 8000,
            amount_received: null,
            incremental_authorization_allowed: true,
            authorization_count: 1,
            latest_authorization: {
              amount: 8000,
              previously_authorized_amount: 6000,
              status: "success",
              error_code: null,
              error_message: null,
            },
          }),
        },
        ({ actual, expected }) => expect(actual).toMatchObject(expected),
      );
    });

    await report.step("Capture the increased amount", async () => {
      const response = await payments.capturePayment(
        "capture-increased-authorization",
        payment.payment_id,
        { amount_to_capture: 8000 },
      );
      await report.check(
        "Incremental authorization capture fields",
        {
          actual: select(response, [
            "payment_id",
            "status",
            "amount",
            "amount_capturable",
            "amount_received",
            "currency",
          ]),
          expected: expected("incremental-authorization-capture-fields", {
            payment_id: payment.payment_id,
            status: "succeeded",
            amount: 8000,
            amount_capturable: 0,
            amount_received: 8000,
            currency: "USD",
          }),
        },
        ({ actual, expected }) => expect(actual).toMatchObject(expected),
      );
    });

    const recorded = await payments.readRecordedPayment(payment.payment_id);
    await report.check(
      "Recorded incremental authorization outcome",
      {
        actual: recorded,
        expected: expected("incremental-authorization-recorded-outcome", {
          gateway: "unified_connector_service",
          attemptStatus: "charged",
          connectorTransactionIdPresent: true,
          customerRecordExists: true,
        }),
      },
      ({ actual, expected }) => expect(actual).toEqual(expected),
    );
  },
);

function select(response: Record<string, unknown>, fields: string[]) {
  return Object.fromEntries(fields.map((field) => [field, response[field]]));
}
