import { test } from "@playwright/test"
import { hasConnector } from "../fixtures/connectors"
import { initiateCheckout, waitForOrderPage, OrderPage } from "../fixtures/pages"
import { fillStripeCard } from "../fixtures/iframes"

// The Card Element + confirmCardPayment path (Medusa's official integration) is
// not gated behind hCaptcha — unlike the Payment Element's confirmPayment —, so
// it completes under automation. Runs whenever Stripe creds are present.
//
// This UCS/Prism Stripe account creates the PaymentIntent with automatic
// capture, so confirming the card charges immediately and the order lands
// CAPTURED (like PayPal) — refund only, no separate capture/void step.
const enabled = hasConnector("stripe")

test.describe("stripe checkout (card element)", () => {
  test.skip(!enabled, "stripe credentials not present in creds.json")

  test("fill card → confirm (captured) → refund", async ({ page }) => {
    await initiateCheckout(page, "stripe")
    await fillStripeCard(page)
    await waitForOrderPage(page)

    const order = new OrderPage(page)
    await order.expectStatusOneOf("captured", "succeeded")
    await order.refund()
  })
})
