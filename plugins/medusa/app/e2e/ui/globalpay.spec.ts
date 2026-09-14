import { test } from "@playwright/test"
import { hasConnector } from "../fixtures/connectors"
import { initiateCheckout, waitForOrderPage, OrderPage } from "../fixtures/pages"
import { fillGlobalpayCard } from "../fixtures/iframes"

const enabled = hasConnector("globalpay")

test.describe("globalpay checkout", () => {
  test.skip(!enabled, "globalpay credentials not present in creds.json")

  // GlobalPay auto-captures at authorize time → lands CAPTURED (refund only).
  test("tokenize → authorize (captured) → refund", async ({ page }) => {
    await initiateCheckout(page, "globalpay")
    await fillGlobalpayCard(page) // tokenize → onSubmit reinitiates → navigates
    await waitForOrderPage(page)

    const order = new OrderPage(page)
    await order.expectStatusOneOf("captured", "succeeded")
    await order.refund()
  })
})
