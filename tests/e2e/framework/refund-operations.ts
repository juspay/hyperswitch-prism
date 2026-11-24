import { test } from "@playwright/test";
import { HyperswitchClient, stringField } from "./hyperswitch-client.js";
import type { TestMerchant } from "./merchant-setup.js";
import {
  readRecordedRefund,
  readRecordedRefundReversal,
} from "./refund-records.js";
import type { ScenarioOverrideResolver } from "./scenario-overrides.js";
import type { Json, JsonObject } from "./types.js";

export type RefundResponse = Json & { refund_id: string; status: string };

export class RefundOperations {
  constructor(
    private readonly hs: HyperswitchClient,
    private readonly merchant: TestMerchant,
    private readonly overrides: ScenarioOverrideResolver,
  ) {}

  async createRefund(
    callId: string,
    request: JsonObject,
  ): Promise<RefundResponse> {
    return refundResponse(
      await this.hs.call(
        "POST",
        "/refunds",
        this.merchant.apiKey,
        this.overrides.request("createRefund", callId, request),
      ),
    );
  }

  async reverseRefund(
    callId: string,
    refundId: string,
    request: JsonObject,
  ): Promise<RefundResponse> {
    return refundResponse(
      await this.hs.call(
        "POST",
        `/refunds/${encodeURIComponent(refundId)}/reverse`,
        this.merchant.apiKey,
        this.overrides.request("reverseRefund", callId, request),
      ),
    );
  }

  async readRecordedRefund(refundId: string, paymentId: string) {
    return test.step("Read recorded refund from HS database", () =>
      readRecordedRefund(this.merchant.merchantId, refundId, paymentId));
  }

  async readRecordedRefundReversal(refundId: string, paymentId: string) {
    return test.step("Read recorded refund-reversal metadata from HS database", () =>
      readRecordedRefundReversal(
        this.merchant.merchantId,
        refundId,
        paymentId,
      ));
  }
}

function refundResponse(response: Json): RefundResponse {
  return {
    ...response,
    refund_id: stringField(response, "refund_id"),
    status: stringField(response, "status"),
  };
}
