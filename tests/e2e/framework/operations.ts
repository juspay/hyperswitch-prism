import { test } from "@playwright/test";
import { Checks } from "./checks.js";
import { Hyperswitch, type Json } from "./hyperswitch.js";
import type { TestMerchant } from "./merchant-setup.js";
import type { ConnectorDefinition } from "./scenario.js";
import { readPaymentRoute } from "./payment-records.js";

export type PaymentResponse = Json & { payment_id: string; status: string };

// Each method performs one named operation; the scenario chooses their order.
// Request fields are forwarded unchanged. HS owns their validation.
export class PaymentOperations {
  constructor(
    private readonly hs: Hyperswitch,
    private readonly merchant: TestMerchant,
    private readonly connector: ConnectorDefinition,
    private readonly checks: Checks,
  ) {}

  async createPayment(request: Json): Promise<PaymentResponse> {
    const response = await this.hs.call(
      "POST",
      "/payments",
      this.merchant.apiKey,
      request,
    );
    if (
      typeof response.payment_id !== "string" ||
      !response.payment_id ||
      typeof response.status !== "string"
    ) {
      throw new Error("Create payment must return payment_id and status");
    }
    return response as PaymentResponse;
  }

  async retrievePayment(
    paymentId: string,
    request: { force_sync: boolean },
  ): Promise<PaymentResponse> {
    return (await this.hs.call(
      "GET",
      `/payments/${encodeURIComponent(paymentId)}?force_sync=${request.force_sync}`,
      this.merchant.apiKey,
    )) as PaymentResponse;
  }

  async retrieveUntilStatus(
    paymentId: string,
    request: { force_sync: boolean },
    expectedStatus: string,
  ): Promise<PaymentResponse> {
    return test.step(`Retrieve payment until status is ${expectedStatus}`, async () =>
      (await this.checks.waitForStatus(
        `Payment status: ${expectedStatus}`,
        () => this.retrievePayment(paymentId, request),
        expectedStatus,
      )) as PaymentResponse);
  }

  async readRecordedRouting(paymentId: string, attemptStatus: string) {
    return test.step("Read recorded gateway and processor reference from HS database", () =>
      readPaymentRoute(
        this.merchant.merchantId,
        paymentId,
        this.connector.id,
        attemptStatus,
      ));
  }
}
