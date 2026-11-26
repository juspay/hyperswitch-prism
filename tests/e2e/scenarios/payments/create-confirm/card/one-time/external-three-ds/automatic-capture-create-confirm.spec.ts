import { expect } from "@playwright/test";
import { test } from "../../../../../../framework/fixtures.js";
import { scenarioFromFile } from "../../../../../catalog.js";
import {
  createAddress,
  createBrowserInfo,
  createCustomer,
  createReference,
  testReturnUrl,
} from "../../../../../../test-data.js";

const scenario = scenarioFromFile(import.meta.url, {
  title: "Automatically capture a card payment with external 3DS data",
  description:
    "Submit a payment with a synthetic merchant-supplied 3DS authentication result instead of asking HS to run a challenge.",
  verifies:
    "Verify HS forwards the externally authenticated transaction to UCS, captures it without redirecting, preserves 3DS authentication semantics, and records the processor reference.",
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
    const response = await report.step(
      "Create and confirm with external 3DS proof",
      () =>
        payments.createPayment("external-three-ds-payment", {
          profile_id: merchant.profileId,
          amount: 6000,
          currency: "USD",
          confirm: true,
          authentication_type: "three_ds",
          request_external_three_ds_authentication: true,
          three_ds_data: {
            authentication_cryptogram: {
              cavv: {
                authentication_cryptogram: "3q2+78r+ur7erb7vyv66vv////8=",
              },
            },
            ds_trans_id: "00000000-0000-4000-8000-000000000001",
            version: "2.1.0",
            eci: "05",
            transaction_status: "Y",
            exemption_indicator: "low_value",
          },
          capture_method: "automatic",
          payment_method: "card",
          payment_method_type: "credit",
          payment_method_data: {
            card: {
              card_number: "4242424242424242",
              card_exp_month: "12",
              card_exp_year: "30",
              card_holder_name: "Test Customer",
              card_cvc: "123",
            },
          },
          customer,
          billing: { address: createAddress(), email: customer.email },
          browser_info: createBrowserInfo(),
          description: "Synthetic external 3DS payment",
          metadata: { authentication_source: "external_three_ds" },
          merchant_order_reference_id: createReference("order"),
          return_url: testReturnUrl,
        }),
    );

    await report.check(
      "External 3DS payment fields",
      {
        actual: select(response, [
          "status",
          "amount",
          "currency",
          "amount_capturable",
          "amount_received",
          "authentication_type",
          "capture_method",
          "connector",
          "next_action",
          "error_code",
          "error_message",
        ]),
        expected: expected("external-three-ds-fields", {
          status: "succeeded",
          amount: 6000,
          currency: "USD",
          amount_capturable: 0,
          amount_received: 6000,
          authentication_type: "three_ds",
          capture_method: "automatic",
          connector: connector.id,
          next_action: null,
          error_code: null,
          error_message: null,
        }),
      },
      ({ actual, expected }) => expect(actual).toMatchObject(expected),
    );

    const recorded = await payments.readRecordedPayment(response.payment_id);
    await report.check(
      "Recorded external 3DS outcome",
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
  },
);

function select(response: Record<string, unknown>, fields: string[]) {
  return Object.fromEntries(fields.map((field) => [field, response[field]]));
}
