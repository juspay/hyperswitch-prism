import { test, Page, APIRequestContext } from "@playwright/test"
import {
  hasConnector,
  adyenHmacKey,
  adyenHmacKeyReady,
  adyenMerchantAccount,
} from "../fixtures/connectors"
import { initiateCheckout, waitForOrderPage, OrderPage } from "../fixtures/pages"
import { fillAdyenCard } from "../fixtures/iframes"
import { buildAdyenNotification, injectAdyenWebhook } from "../fixtures/adyen-webhook"

const enabled = hasConnector("adyen") && adyenHmacKeyReady()

// Adyen's sessions flow authorizes client-side via the drop-in; the outcome
// reaches the merchant ONLY by webhook, which the real sandbox cannot deliver
// to localhost. We reproduce it with a synthetic, genuinely HMAC-signed
// AUTHORISATION notification: the test signs with adyen.hmac_key and the server
// verifies with the same key (a closed loop), so this exercises the real Adyen
// integration logic — HMAC source verification, ParseEvent merchantReference
// correlation, and the authorize poll reading the recorded webhook outcome.
//
// Settlement (capture / refund / void) is intentionally NOT asserted here:
// those call the live Adyen API and require the payment's REAL pspReference,
// which sessions flow never exposes client-side and which only ever arrives via
// the real webhook. A synthetic pspReference is rejected by Adyen ("Original
// pspReference required"). Settlement is a generic UCS operation already covered
// end-to-end by the Stripe / GlobalPay / PayPal specs (which have real refs).
async function authorizeAdyen(
  page: Page,
  request: APIRequestContext
): Promise<OrderPage> {
  const { data } = await initiateCheckout(page, "adyen")
  await fillAdyenCard(page)

  await injectAdyenWebhook(
    request,
    buildAdyenNotification({
      merchantReference: data.merchantClientSessionId,
      pspReference: `psp_test_${Date.now()}`,
      merchantAccountCode: adyenMerchantAccount()!,
      amountValue: data.minorAmount,
      currency: data.currency,
      eventCode: "AUTHORISATION",
      success: true,
      hmacKeyHex: adyenHmacKey()!,
    })
  )

  await waitForOrderPage(page)
  const order = new OrderPage(page)
  await order.expectStatusOneOf("authorized")
  return order
}

test.describe("adyen checkout", () => {
  test.skip(
    !enabled,
    "adyen creds or a real adyen.hmac_key (hex, not the placeholder) missing in creds.json"
  )

  test("fill card → authorize (via HMAC-signed webhook)", async ({
    page,
    request,
  }) => {
    await authorizeAdyen(page, request)
  })
})
