import { test } from "@playwright/test"
import { hasConnector } from "../fixtures/connectors"
import { initiateCheckout, waitForOrderPage, OrderPage } from "../fixtures/pages"
import { fillPaypalCard } from "../fixtures/iframes"

// With the Advanced Card Fields integration the card path is inline hosted
// fields (no popup, no sandbox buyer needed). Requires the PayPal account to
// have Advanced Credit & Debit Card Payments enabled (CardFields eligible).
const enabled = hasConnector("paypal")

test.describe("paypal checkout (card fields)", () => {
  test.skip(!enabled, "paypal credentials not present in creds.json")

  // PayPal auto-captures → lands CAPTURED (refund only).
  test("fill card → authorize (captured) → refund", async ({ page }) => {
    await initiateCheckout(page, "paypal")
    await fillPaypalCard(page) // inline CardFields → submit → navigates
    await waitForOrderPage(page)

    const order = new OrderPage(page)
    await order.expectStatusOneOf("captured", "succeeded")
    await order.refund()
  })
})
