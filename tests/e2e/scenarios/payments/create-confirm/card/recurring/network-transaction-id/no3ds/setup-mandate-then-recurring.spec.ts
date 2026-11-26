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
  title: "No3DS card recurring payment — card details with network transaction ID",
  description:
    "Make a customer-initiated card payment for future off-session use, obtain the real network transaction reference recorded by HS, then make a merchant-initiated recurring payment using the same card details and that reference.",
  verifies:
    "Verify the initial payment succeeds and records a nonempty network transaction reference; then verify a distinct off-session payment succeeds through the card-with-NTI path without using a connector mandate or a network token.",
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
    const card = createCard(connector.card);
    const billingAddress = createAddress();
    const shippingAddress = createAddress({ line1: "2" });
    const customer = createCustomer();
    const phone = {
      number: customer.phone,
      country_code: customer.phone_country_code,
    };
    const browserInfo = createBrowserInfo();
    const customerAcceptance = createCustomerAcceptance();
    const setupMetadata = { payment_purpose: "network_reference_setup" };
    const setupDescription = "Synthetic network reference setup";
    const setupOrderReference = createReference("order");
    const setupOrderDetails = [createOrderItem({ quantity: 2, amount: 3000 })];

    const setupPayment = await report.step(
      "Complete the customer-initiated payment and request future off-session use",
      async () => {
        const response = await payments.createPayment("network-reference-setup", {
          profile_id: merchant.profileId,
          amount: 6000,
          currency: "USD",
          confirm: true,
          authentication_type: "no_three_ds",
          capture_method: "automatic",
          payment_method: "card",
          payment_method_type: "credit",
          payment_method_data: { card },
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
          description: setupDescription,
          metadata: setupMetadata,
          merchant_order_reference_id: setupOrderReference,
          order_details: setupOrderDetails,
          return_url: testReturnUrl,
        });

        const expectedSetup = expected("network-reference-setup-fields", {
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
          "Network-reference setup payment fields",
          {
            actual: Object.fromEntries(
              Object.keys(expectedSetup).map((field) => [field, response[field]]),
            ),
            expected: expectedSetup,
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );
        await report.check(
          "Network-reference setup customer and order details",
          {
            actual: {
              billing: response.billing,
              shipping: response.shipping,
              customer: response.customer,
              browser_info: response.browser_info,
              order_details: response.order_details,
            },
            expected: expected("network-reference-setup-details", {
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
          "Initial payment references",
          {
            actual: {
              paymentIdPresent: nonempty(response.payment_id),
              connectorTransactionIdPresent: nonempty(
                response.connector_transaction_id,
              ),
              networkTransactionIdPresent: nonempty(
                response.network_transaction_id,
              ),
            },
            expected: {
              paymentIdPresent: true,
              connectorTransactionIdPresent: true,
              networkTransactionIdPresent: true,
            },
          },
          ({ actual, expected }) => expect(actual).toEqual(expected),
        );
        return response;
      },
    );

    const networkTransactionId = requiredString(
      setupPayment.network_transaction_id,
      "network_transaction_id",
    );
    const recordedReference = await payments.readRecordedNetworkTransactionReference(
      setupPayment.payment_id,
    );
    await report.check(
      "Recorded network transaction reference",
      {
        actual: {
          networkTransactionIdPresent: nonempty(
            recordedReference.networkTransactionId,
          ),
          matchesPaymentResponse:
            recordedReference.networkTransactionId === networkTransactionId,
        },
        expected: {
          networkTransactionIdPresent: true,
          matchesPaymentResponse: true,
        },
      },
      ({ actual, expected }) => expect(actual).toEqual(expected),
    );

    const recurringMetadata = { payment_purpose: "network_reference_recurring" };
    const recurringDescription = "Synthetic card-with-NTI recurring charge";
    const recurringOrderReference = createReference("order");
    const recurringOrderDetails = [createOrderItem({ amount: 2500 })];
    const recurringPayment = await report.step(
      "Charge the card off-session using its network transaction reference",
      async () => {
        const recurringData = {
          card_number: card.card_number,
          card_exp_month: card.card_exp_month,
          card_exp_year: card.card_exp_year,
          card_holder_name: card.card_holder_name,
          network_transaction_id: networkTransactionId,
          ...(recordedReference.transactionLinkId
            ? { transaction_link_id: recordedReference.transactionLinkId }
            : {}),
        };
        const response = await payments.createPayment("card-with-nti-recurring", {
          profile_id: merchant.profileId,
          amount: 2500,
          currency: "USD",
          confirm: true,
          authentication_type: "no_three_ds",
          capture_method: "automatic",
          payment_method: "card",
          payment_method_type: "credit",
          customer_id: customer.id,
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
          recurring_details: {
            type: "network_transaction_id_and_card_details",
            data: recurringData,
          },
          off_session: true,
          description: recurringDescription,
          metadata: recurringMetadata,
          merchant_order_reference_id: recurringOrderReference,
          order_details: recurringOrderDetails,
          return_url: testReturnUrl,
        });

        const expectedRecurring = expected("card-with-nti-recurring-fields", {
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
          payment_method_type: null,
          authentication_type: "no_three_ds",
          amount: 2500,
          net_amount: 2500,
          amount_capturable: 0,
          currency: "USD",
          amount_received: 2500,
          setup_future_usage: null,
          next_action: null,
          mandate_id: null,
          attempt_count: 1,
          email: customer.email,
          customer_id: customer.id,
          name: customer.name,
          phone: customer.phone,
          description: recurringDescription,
          metadata: recurringMetadata,
          merchant_order_reference_id: recurringOrderReference,
        });
        await report.check(
          "Card-with-NTI recurring payment fields",
          {
            actual: Object.fromEntries(
              Object.keys(expectedRecurring).map((field) => [field, response[field]]),
            ),
            expected: expectedRecurring,
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );
        await report.check(
          "Card-with-NTI recurring customer and order details",
          {
            actual: {
              billing: response.billing,
              shipping: response.shipping,
              customer: response.customer,
              browser_info: response.browser_info,
              order_details: response.order_details,
            },
            expected: expected("card-with-nti-recurring-details", {
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
              order_details: recurringOrderDetails,
            }),
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );
        await report.check(
          "Card-with-NTI recurring payment references",
          {
            actual: {
              paymentIdPresent: nonempty(response.payment_id),
              newPaymentCreated: response.payment_id !== setupPayment.payment_id,
              connectorTransactionIdPresent: nonempty(
                response.connector_transaction_id,
              ),
            },
            expected: {
              paymentIdPresent: true,
              newPaymentCreated: true,
              connectorTransactionIdPresent: true,
            },
          },
          ({ actual, expected }) => expect(actual).toEqual(expected),
        );
        return response;
      },
    );

    const recordedRecurring = await payments.readRecordedPayment(
      recurringPayment.payment_id,
    );
    await report.check(
      "Recorded card-with-NTI recurring payment",
      {
        actual: recordedRecurring,
        expected: expected("recorded-card-with-nti-recurring-payment", {
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

function nonempty(value: unknown): boolean {
  return typeof value === "string" && value.trim().length > 0;
}

function requiredString(value: unknown, field: string): string {
  if (!nonempty(value)) throw new Error(`Response must contain ${field}`);
  return value as string;
}
