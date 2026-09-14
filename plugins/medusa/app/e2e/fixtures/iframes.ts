import { Page, FrameLocator, expect } from "@playwright/test"
import { TEST_CARDS } from "./test-cards"

// NOTE: third-party payment UIs render hosted fields inside cross-origin iframes
// whose internal structure is owned by the connector and can change. The
// selectors below are best-effort against current SDK versions; if a connector
// ships a markup change, update the frame/field locators here in one place.

/**
 * Stripe Card Element — the all-in-one CardElement renders a single combined
 * iframe titled "Secure card payment input frame" with inputs named
 * `cardnumber` / `exp-date` / `cvc` (postal hidden via hidePostalCode). Typing
 * the number auto-advances focus, but filling each named input directly is
 * stable here. StripeWrapper renders its own Pay button outside the iframe.
 */
export async function fillStripeCard(page: Page): Promise<void> {
  const card = TEST_CARDS.stripe
  await expect(page.locator(".stripe-card-element")).toBeVisible()

  const frame: FrameLocator = page
    .locator('iframe[title="Secure card payment input frame"]')
    .first()
    .contentFrame()
  await frame.locator('input[name="cardnumber"]').fill(card.number)
  await frame.locator('input[name="exp-date"]').fill(`${card.expMonth} / ${card.expYear}`)
  await frame.locator('input[name="cvc"]').fill(card.cvc)

  await page.locator(".stripe-pay-button").click()
}

/**
 * Adyen secured fields — each field is its own titled iframe; the real input
 * carries an id containing the encrypted field name (the others are a11y shims).
 */
export async function fillAdyenCard(page: Page): Promise<void> {
  const card = TEST_CARDS.adyen
  await expect(page.locator(".adyen-payment-container")).toBeVisible()

  await page
    .frameLocator('iframe[title="Card number"]')
    .locator('input[id*="encryptedCardNumber"]')
    .fill(card.number)
  await page
    .frameLocator('iframe[title="Expiry date"]')
    .locator('input[id*="encryptedExpiryDate"]')
    .fill(`${card.expMonth}/${card.expYear}`)
  await page
    .frameLocator('iframe[title="Security code"]')
    .locator('input[id*="encryptedSecurityCode"]')
    .fill(card.cvc)

  // Adyen's SDK renders the Pay button (label includes the amount).
  await page.getByRole("button", { name: /pay \$/i }).click()
}

/**
 * GlobalPay hosted fields — separate titled iframes; the visible input in each
 * is #secure-payment-field. The form renders its own Submit button (in a frame).
 */
export async function fillGlobalpayCard(page: Page): Promise<void> {
  const card = TEST_CARDS.globalpay
  await expect(page.locator(".globalpay-payment-container")).toBeVisible()

  await page
    .frameLocator('iframe[title="Card Number Input"]')
    .locator("#secure-payment-field")
    .fill(card.number)
  // Expiration field placeholder is "MM / YYYY".
  await page
    .frameLocator('iframe[title="Card Expiration Input"]')
    .locator("#secure-payment-field")
    .fill(`${card.expMonth} / 20${card.expYear}`)
  await page
    .frameLocator('iframe[title="Card CVV Input"]')
    .locator("#secure-payment-field")
    .fill(card.cvc)
  await page
    .frameLocator('iframe[title="Card Holder Name Input"]')
    .locator("#secure-payment-field")
    .fill("Test Holder")

  // Submitting tokenizes the card; the wrapper then reinitiates + navigates,
  // so we do NOT wait for the "Card saved" message (the page navigates away).
  await page
    .frameLocator('iframe[title="Form Submit Button Input"]')
    .locator("button")
    .click()
}

/**
 * PayPal Advanced Card Fields — inline hosted fields (no popup, no billing
 * block). Fills the number/expiry/cvv iframes and clicks the custom Pay button,
 * which calls cardFields.submit() → onApprove → the authorize flow.
 */
export async function fillPaypalCard(page: Page): Promise<void> {
  const card = TEST_CARDS.paypal
  await expect(page.locator(".paypal-card-fields")).toBeVisible()

  // The expiry/cvv inputs are mask-formatted and validate on keystroke, so a
  // direct .fill() is rejected (INVALID_EXPIRY / INVALID_CVV) — type those.
  // The number field is filled directly: typing it auto-advances focus and
  // corrupts the expiry entry.
  const type = async (frameTitle: string, inputName: string, value: string) => {
    const field = page
      .frameLocator(`iframe[title="${frameTitle}"]`)
      .locator(`input[name="${inputName}"]`)
    await field.click()
    // The cross-origin field drops early keystrokes until it has settled, and
    // pressSequentially can mangle the mask — focus then drive the keyboard
    // directly, one digit at a time with generous spacing.
    await page.waitForTimeout(600)
    for (const ch of value) {
      await page.keyboard.press(ch)
      await page.waitForTimeout(160)
    }
  }

  await page
    .frameLocator('iframe[title="paypal_card_number_field"]')
    .locator('input[name="number"]')
    .fill(card.number)
  await type("paypal_card_expiry_field", "expiry", `${card.expMonth}${card.expYear}`)
  await type("paypal_card_cvv_field", "cvv", card.cvc)

  await page.locator(".paypal-card-submit").click()
}
