import { expect, type Page } from "@playwright/test";
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
import type {
  Json,
  ThreeDsChallenge,
} from "../../../../../../framework/types.js";

const scenario = scenarioFromFile(import.meta.url, {
  title: "One-time 3DS card — automatic capture, inline confirmation",
  description:
    "Create and confirm a customer-present 3DS card payment, complete the processor-hosted browser challenge, then verify the payment finishes with automatic capture.",
  verifies:
    "Verify the exact customer-action precondition and redirect action, successful browser authentication, final charged response, payment identity, ownership, amounts and recorded UCS outcome.",
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
  async ({ connector, merchant, payments, report, expected, page }) => {
    if (!connector.threeDsChallenge) {
      throw new Error(
        "Connector must configure a synthetic 3DS challenge before enabling this scenario",
      );
    }
    const billingAddress = createAddress();
    const shippingAddress = createAddress({ line1: "2" });
    const customer = createCustomer();
    const phone = {
      number: customer.phone,
      country_code: customer.phone_country_code,
    };
    const browserInfo = createBrowserInfo();
    const metadata = { order_type: "one_time", authentication: "three_ds" };
    const description = "Synthetic 3DS order";
    const orderReference = createReference("order");
    const orderDetails = [createOrderItem({ quantity: 2, amount: 3000 })];

    const payment = await report.step(
      "Create and confirm the 3DS payment",
      async () => {
        const response = await payments.createPayment("three-ds-payment", {
          profile_id: merchant.profileId,
          amount: 6000,
          currency: "USD",
          confirm: true,
          authentication_type: "three_ds",
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
          description,
          metadata,
          merchant_order_reference_id: orderReference,
          order_details: orderDetails,
          return_url: testReturnUrl,
        });
        const expectedPreChallenge = expected("pre-challenge-fields", {
          status: "requires_customer_action",
          error_code: null,
          error_message: null,
          merchant_id: merchant.merchantId,
          profile_id: merchant.profileId,
          connector: connector.id,
          merchant_connector_id: merchant.merchantConnectorId,
          capture_method: "automatic",
          payment_method: "card",
          payment_method_type: "credit",
          authentication_type: "three_ds",
          amount: 6000,
          net_amount: 6000,
          amount_capturable: 6000,
          currency: "USD",
          amount_received: null,
          setup_future_usage: null,
          mandate_id: null,
          attempt_count: 1,
          customer_id: customer.id,
          description,
          metadata,
          merchant_order_reference_id: orderReference,
        });
        await report.check(
          "Pre-challenge payment fields",
          {
            actual: Object.fromEntries(
              Object.keys(expectedPreChallenge).map((field) => [
                field,
                response[field],
              ]),
            ),
            expected: expectedPreChallenge,
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );
        await report.check(
          "Pre-challenge customer and order details",
          {
            actual: {
              billing: response.billing,
              shipping: response.shipping,
              customer: response.customer,
              browser_info: response.browser_info,
              order_details: response.order_details,
            },
            expected: expected("pre-challenge-customer-and-order-details", {
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
        await report.check(
          "Pre-challenge return URL",
          {
            actual:
              typeof response.return_url === "string"
                ? new URL(response.return_url).href
                : null,
            expected: new URL(testReturnUrl).href,
          },
          ({ actual, expected }) => expect(actual).toBe(expected),
        );
        await report.check(
          "3DS redirect action",
          {
            actual: {
              nextActionPresent: isObject(response.next_action),
              actionType: isObject(response.next_action)
                ? response.next_action.type
                : null,
              redirectUrlPresent: nonempty(redirectUrl(response)),
            },
            expected: {
              nextActionPresent: true,
              actionType: "redirect_to_url",
              redirectUrlPresent: true,
            },
          },
          ({ actual, expected }) => expect(actual).toEqual(expected),
        );
        return response;
      },
    );

    await report.step(
      "Complete the processor-hosted 3DS challenge",
      async () => {
        await page.goto(requiredRedirectUrl(payment));
        await completeChallenge(page, connector.threeDsChallenge!);
        await page.waitForURL(
          (url) => url.origin === new URL(testReturnUrl).origin,
          {
            timeout: 90_000,
          },
        );
      },
    );

    const completed = await report.step(
      "Retrieve the completed 3DS payment",
      async () => {
        const response = await payments.retrievePayment(payment.payment_id, {
          force_sync: false,
        });
        const expectedCompleted = expected("completed-three-ds-fields", {
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
          authentication_type: "three_ds",
          amount: 6000,
          net_amount: 6000,
          amount_capturable: 0,
          currency: "USD",
          amount_received: 6000,
          setup_future_usage: null,
          next_action: null,
          mandate_id: null,
          attempt_count: 1,
          customer_id: customer.id,
          description,
          metadata,
          merchant_order_reference_id: orderReference,
        });
        await report.check(
          "Completed 3DS payment fields",
          {
            actual: Object.fromEntries(
              Object.keys(expectedCompleted).map((field) => [
                field,
                response[field],
              ]),
            ),
            expected: expectedCompleted,
          },
          ({ actual, expected }) => expect(actual).toMatchObject(expected),
        );
        await report.check(
          "Completed 3DS payment references",
          {
            actual: {
              samePayment: response.payment_id === payment.payment_id,
              connectorTransactionIdPresent: nonempty(
                response.connector_transaction_id,
              ),
            },
            expected: {
              samePayment: true,
              connectorTransactionIdPresent: true,
            },
          },
          ({ actual, expected }) => expect(actual).toEqual(expected),
        );
        return response;
      },
    );

    await report.step("Verify the recorded completed 3DS payment", async () => {
      const recorded = await payments.readRecordedPayment(completed.payment_id);
      await report.check(
        "Recorded completed 3DS payment",
        {
          actual: recorded,
          expected: expected("recorded-completed-three-ds-payment", {
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

async function completeChallenge(
  page: Page,
  challenge: ThreeDsChallenge,
): Promise<void> {
  const root = challenge.frameSelector
    ? page.frameLocator(challenge.frameSelector)
    : page;
  if (challenge.username) {
    const username = challenge.usernameSelector
      ? root.locator(challenge.usernameSelector)
      : root
          .getByLabel(/user(name)?/i)
          .or(root.locator('input[name="username"]'));
    await username.first().fill(challenge.username);
  }
  const password = challenge.passwordSelector
    ? root.locator(challenge.passwordSelector)
    : root.getByLabel(/password/i).or(root.locator('input[type="password"]'));
  await password.first().fill(challenge.password);
  const submit = challenge.submitSelector
    ? root.locator(challenge.submitSelector)
    : root
        .getByRole("button", { name: /submit|authenticate|continue|complete/i })
        .or(root.locator('button[type="submit"], input[type="submit"]'));
  await submit.first().click();
}

function isObject(value: unknown): value is Json {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function redirectUrl(response: Json): unknown {
  return isObject(response.next_action)
    ? response.next_action.redirect_to_url
    : undefined;
}

function requiredRedirectUrl(response: Json): string {
  const value = redirectUrl(response);
  if (!nonempty(value)) {
    throw new Error("Payment response must contain a 3DS redirect URL");
  }
  return value as string;
}

function nonempty(value: unknown): boolean {
  return typeof value === "string" && value.trim().length > 0;
}
