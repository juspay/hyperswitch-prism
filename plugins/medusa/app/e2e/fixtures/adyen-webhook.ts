import crypto from "crypto"
import { APIRequestContext, expect } from "@playwright/test"

// The harness webhook route derives the connector from the last hyphen segment.
const ADYEN_WEBHOOK_PATH =
  "/hooks/payment/hyperswitch-prism_hyperswitch-prism-adyen"

export interface AdyenNotificationParams {
  merchantReference: string // must equal the session's merchantClientSessionId
  pspReference: string
  merchantAccountCode: string
  amountValue: number // minor units
  currency: string
  eventCode?: string // default AUTHORISATION
  success?: boolean // default true
  hmacKeyHex: string
}

// Adyen's HMAC: escape `\` and `:` in each value, join the signed fields with
// `:`, HMAC-SHA256 with the hex key, base64-encode.
function adyenHmacSignature(
  item: Record<string, any>,
  hmacKeyHex: string
): string {
  const escape = (v: unknown) =>
    String(v ?? "").replace(/\\/g, "\\\\").replace(/:/g, "\\:")
  const signedValues = [
    item.pspReference,
    item.originalReference,
    item.merchantAccountCode,
    item.merchantReference,
    item.amount?.value,
    item.amount?.currency,
    item.eventCode,
    item.success,
  ].map(escape)
  const dataToSign = signedValues.join(":")
  const key = Buffer.from(hmacKeyHex, "hex")
  return crypto.createHmac("sha256", key).update(dataToSign, "utf8").digest("base64")
}

export function buildAdyenNotification(p: AdyenNotificationParams) {
  const item: Record<string, any> = {
    amount: { currency: p.currency, value: p.amountValue },
    eventCode: p.eventCode ?? "AUTHORISATION",
    eventDate: new Date().toISOString(),
    merchantAccountCode: p.merchantAccountCode,
    merchantReference: p.merchantReference,
    originalReference: "",
    pspReference: p.pspReference,
    success: String(p.success ?? true),
  }
  item.additionalData = { hmacSignature: adyenHmacSignature(item, p.hmacKeyHex) }
  return {
    live: "false",
    notificationItems: [{ NotificationRequestItem: item }],
  }
}

/** POSTs a signed notification to the harness Adyen webhook endpoint. */
export async function injectAdyenWebhook(
  request: APIRequestContext,
  notification: ReturnType<typeof buildAdyenNotification>
): Promise<void> {
  const res = await request.post(ADYEN_WEBHOOK_PATH, {
    headers: { "Content-Type": "application/json" },
    data: notification,
  })
  // The route returns the WebhookActionResult; 2xx means it was verified+mapped.
  expect(res.ok(), `webhook rejected: ${res.status()} ${await res.text()}`).toBeTruthy()
}
