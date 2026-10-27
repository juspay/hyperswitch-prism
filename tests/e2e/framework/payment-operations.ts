import { test } from "@playwright/test";
import { Assertions } from "./assertions.js";
import { HyperswitchClient, stringField } from "./hyperswitch-client.js";
import type { TestMerchant } from "./merchant-setup.js";
import type { Json, ScenarioOverrides } from "./types.js";
import { readRecordedPayment } from "./payment-records.js";

export type PaymentResponse = Json & { payment_id: string; status: string };

// Each method performs one named operation; the scenario chooses their order.
// Apply connector request differences once, then forward to HS for validation.
export class PaymentOperations {
  constructor(
    private readonly hs: HyperswitchClient,
    private readonly merchant: TestMerchant,
    private readonly checks: Assertions,
    private readonly overrides: ScenarioOverrides,
  ) {}

  async createPayment(request: Json): Promise<PaymentResponse> {
    const response = await this.hs.call(
      "POST",
      "/payments",
      this.merchant.apiKey,
      { ...request, ...this.overrides.requests?.createPayment },
    );
    return paymentResponse(response);
  }

  async retrievePayment(
    paymentId: string,
    request: { force_sync: boolean },
  ): Promise<PaymentResponse> {
    return paymentResponse(
      await this.hs.call(
        "GET",
        `/payments/${encodeURIComponent(paymentId)}?force_sync=${request.force_sync}`,
        this.merchant.apiKey,
      ),
    );
  }

  async waitForPaymentStatus(
    paymentId: string,
    request: { force_sync: boolean },
    expectedStatus: string,
  ): Promise<PaymentResponse> {
    return test.step(`Wait for payment status: ${expectedStatus}`, async () =>
      paymentResponse(
        await this.checks.waitForStatus(
          `Payment status: ${expectedStatus}`,
          () => this.retrievePayment(paymentId, request),
          expectedStatus,
        ),
      ));
  }

  async readRecordedPayment(paymentId: string) {
    return test.step("Read recorded payment and active attempt from HS database", () =>
      readRecordedPayment(this.merchant.merchantId, paymentId));
  }
}

function paymentResponse(response: Json): PaymentResponse {
  return {
    ...response,
    payment_id: stringField(response, "payment_id"),
    status: stringField(response, "status"),
  };
}
