import { test, type TestInfo } from "@playwright/test";
import type { ConnectorDefinition, PaymentScenario } from "./scenario.js";
import { Hyperswitch, type Json } from "./hyperswitch.js";
import { setupMerchant } from "./merchant-setup.js";
import { readPaymentRoute } from "./payment-records.js";
import { verifyChecks, waitForStatus } from "./checks.js";

// The spec supplies the operation order, request bodies, and expectations.
// The runner dispatches operations and resolves IDs; HTTP, DB, and checks have dedicated helpers.
export async function runScenario(
  connector: ConnectorDefinition,
  scenario: PaymentScenario,
  info: TestInfo,
): Promise<void> {
  info.annotations.push({
    type: "Scenario",
    description: scenario.description,
  });
  const hs = await Hyperswitch.create();
  const payments = new Map<string, string>();
  try {
    const merchant = await setupMerchant(hs, connector);
    const paymentId = (name: string): string => {
      const id = payments.get(name);
      if (!id) throw new Error(`Payment '${name}' must be created before use`);
      return id;
    };

    for (const step of scenario.steps) {
      await test.step(step.name, async () => {
        let actual: Json;
        switch (step.action) {
          case "createPayment": {
            actual = await hs.call("POST", "/payments", merchant.apiKey, {
              ...step.request,
              payment_method_data: {
                ...step.request.payment_method_data,
                card: connector.card,
              },
              profile_id: merchant.profileId,
            });
            if (typeof actual.payment_id !== "string" || !actual.payment_id) {
              throw new Error(
                "Create payment did not return a payment identifier",
              );
            }
            payments.set(step.saveAs, actual.payment_id);
            break;
          }
          case "retrievePayment": {
            const path = `/payments/${paymentId(step.payment)}?force_sync=${step.request.force_sync}`;
            actual = await waitForStatus(
              step.name,
              () => hs.call("GET", path, merchant.apiKey),
              step.waitForStatus,
              info,
            );
            break;
          }
          case "verifyPaymentRoute":
            actual = await readPaymentRoute(
              merchant.merchantId,
              paymentId(step.payment),
              connector.id,
              step.request.attempt_status,
            );
            break;
        }
        await verifyChecks(
          step.name,
          step.expect,
          actual,
          {
            "connector.id": connector.id,
            "merchant.merchantConnectorId": merchant.merchantConnectorId,
          },
          info,
        );
      });
    }
  } finally {
    await hs.dispose();
  }
}
