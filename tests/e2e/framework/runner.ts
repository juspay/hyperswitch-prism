import { expect, test, type TestInfo } from "@playwright/test";
import type {
  ConnectorDefinition,
  OneTimeCardScenario,
} from "./scenario.js";
import { Hyperswitch, readPaymentRoute, type Json } from "./hyperswitch.js";
import { setupMerchant } from "./merchant-setup.js";

const browserInfo = {
  accept_header:
    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
  language: "en-US",
  screen_height: 1080,
  screen_width: 1920,
  color_depth: 24,
  user_agent:
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/130.0.0.0 Safari/537.36",
  time_zone: 0,
  java_enabled: false,
  ip_address: "192.0.2.1",
};

async function attachChecks(
  info: TestInfo,
  name: string,
  checks: Record<string, { expected: unknown; actual: unknown }>,
): Promise<void> {
  await info.attach(name, {
    body: JSON.stringify(checks, null, 2),
    contentType: "application/json",
  });
}

function paymentRequest(
  connector: ConnectorDefinition,
  scenario: OneTimeCardScenario,
): Json {
  return {
    amount: scenario.amount,
    currency: scenario.currency,
    confirm: true,
    capture_method: scenario.captureMethod,
    authentication_type: scenario.authentication,
    payment_method: scenario.paymentMethod,
    payment_method_type: "credit",
    payment_method_data: { card: connector.card },
    browser_info: browserInfo,
    billing: {
      address: {
        first_name: "Test",
        last_name: "Customer",
        line1: "1 Test Street",
        city: "San Francisco",
        state: "CA",
        zip: "94107",
        country: "US",
      },
    },
    email: "test@example.com",
    return_url: "https://example.com",
  };
}

export async function runOneTimeCardScenario(
  connector: ConnectorDefinition,
  scenario: OneTimeCardScenario,
  info: TestInfo,
): Promise<void> {
  info.annotations.push(
    { type: "Scenario", description: scenario.description },
    {
      type: "Verifies",
      description:
        "Creation with confirm:true and retrieval succeed with the configured connector through UCS.",
    },
  );

  const hs = await Hyperswitch.create();
  try {
    const merchant = await setupMerchant(hs, connector);
    const request = paymentRequest(connector, scenario);

    const confirmed = await hs.call("POST", "/payments", merchant.apiKey, {
      ...request,
      profile_id: merchant.profileId,
    });
    await test.step("Payment is created and confirmed", async () => {
      expect(typeof confirmed.payment_id, "Payment identifier is recorded").toBe(
        "string",
      );
      await attachChecks(info, "Payment creation checks", {
        status: {
          expected: [scenario.expectedStatus, "processing"],
          actual: confirmed.status,
        },
      });
    });

    const paymentId = confirmed.payment_id as string;
    await test.step("Confirmation succeeds without customer action", async () => {
      expect(
        [scenario.expectedStatus, "processing"],
        "Confirmation status",
      ).toContain(confirmed.status);
      expect(confirmed.next_action ?? null, "No redirect is required").toBeNull();
      expect(confirmed.mandate_id ?? null, "No mandate is created").toBeNull();
      expect(confirmed.connector, "Selected connector").toBe(connector.id);
      expect(
        confirmed.merchant_connector_id,
        "Configured merchant connector account",
      ).toBe(merchant.merchantConnectorId);
    });

    let payment: Json | undefined;
    await test.step(`Verify payment status: ${scenario.expectedStatus}`, async () => {
      await expect
        .poll(
          async () => {
            payment = await hs.call(
              "GET",
              `/payments/${paymentId}?force_sync=true`,
              merchant.apiKey,
            );
            return payment.status;
          },
          { intervals: [1000, 3000, 5000], timeout: 90_000 },
        )
        .toBe(scenario.expectedStatus);

      expect(payment!.connector, "Retrieved connector").toBe(connector.id);
      expect(payment!.merchant_connector_id, "Retrieved connector account").toBe(
        merchant.merchantConnectorId,
      );
      expect(payment!.authentication_type, "Authentication type").toBe(
        scenario.authentication,
      );
      expect(payment!.amount, "Payment amount").toBe(scenario.amount);
      expect(payment!.currency, "Payment currency").toBe(scenario.currency);
      expect(payment!.amount_received, "Amount received").toBe(scenario.amount);
      await attachChecks(info, "Payment result checks", {
        status: {
          expected: scenario.expectedStatus,
          actual: payment!.status,
        },
        connector: { expected: connector.id, actual: payment!.connector },
        authentication: {
          expected: scenario.authentication,
          actual: payment!.authentication_type,
        },
        amount: { expected: scenario.amount, actual: payment!.amount },
        currency: { expected: scenario.currency, actual: payment!.currency },
      });
    });

    await test.step("Verify UCS routing and processor reference", async () => {
      const route = await readPaymentRoute(
        merchant.merchantId,
        paymentId,
        connector.id,
      );
      expect(route.gateway, "Payment gateway").toBe(
        "unified_connector_service",
      );
      expect(route.processorReferenceRecorded, "Processor reference recorded").toBe(
        true,
      );
      await attachChecks(info, "UCS routing checks", {
        gateway: {
          expected: "unified_connector_service",
          actual: route.gateway,
        },
        processor_reference_recorded: {
          expected: true,
          actual: route.processorReferenceRecorded,
        },
      });
    });
  } finally {
    await hs.dispose();
  }
}
