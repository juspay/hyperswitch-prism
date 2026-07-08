import { Page, expect } from "@playwright/test"

export interface InitiatedSession {
  sessionId: string
  data: Record<string, any>
}

/**
 * Navigates to /{connector} and captures the initiated payment session from the
 * POST /payment-sessions response (CheckoutPage initiates it on mount).
 */
export async function initiateCheckout(
  page: Page,
  connector: string
): Promise<InitiatedSession> {
  const waitForSession = page.waitForResponse(
    (r) => r.url().includes("/payment-sessions") && r.request().method() === "POST"
  )
  await page.goto(`/${connector}`)
  const res = await waitForSession
  const body = await res.json()
  const ps = body.payment_collection.payment_sessions[0]
  // The connector UI is mounted only after the session resolves.
  await expect(page.getByTestId(`checkout-${connector}`)).toBeVisible()
  return { sessionId: ps.id as string, data: ps.data as Record<string, any> }
}

/** Waits until the harness has navigated to the order page. */
export async function waitForOrderPage(page: Page): Promise<void> {
  await page.waitForURL(/\/order\//, { timeout: 60_000 })
  await expect(page.getByTestId("order-status")).toBeVisible()
}

export class OrderPage {
  constructor(private readonly page: Page) {}

  async status(): Promise<string> {
    return (await this.page.getByTestId("order-status").innerText()).trim()
  }

  async expectStatusOneOf(...statuses: string[]): Promise<void> {
    await expect
      .poll(async () => (await this.status()).toLowerCase(), { timeout: 30_000 })
      .toMatch(new RegExp(statuses.map((s) => s.toLowerCase()).join("|")))
  }

  async capture(): Promise<void> {
    await this.page.getByTestId("capture-button").click()
    await expect(this.page.getByText(/Captured \(status/)).toBeVisible({ timeout: 60_000 })
  }

  async void(): Promise<void> {
    await this.page.getByTestId("void-button").click()
    await expect(this.page.getByText(/Voided \(status/)).toBeVisible({ timeout: 60_000 })
  }

  async refund(): Promise<void> {
    await this.page.getByTestId("refund-button").click()
    await expect(this.page.getByText(/Refund .* submitted/)).toBeVisible({ timeout: 60_000 })
  }
}
