import fs from "fs"
import path from "path"

// Sandbox/test credentials live in a JSON file (NOT .env), using snake_case
// per-connector keys. This module is the single source of truth for loading
// that file and shaping it for both the integration tests (raw + transforms)
// and the app server (wrapped connector config map).
//
// Resolution order for the file path:
//   1. process.env.CREDS_PATH
//   2. ~/Workspace/creds.json (the conventional location)
//   3. <repo-root>/creds.json
const DEFAULT_CREDS_PATHS = [
  process.env.CREDS_PATH,
  "/Users/jeeva.ramachandran/Workspace/creds.json",
  path.resolve(__dirname, "../../creds.json"),
].filter(Boolean) as string[]

export type RawCreds = Record<string, any>

/** Reads and parses the first creds.json that exists, or null if none found. */
export function loadCredsFile(): RawCreds | null {
  for (const p of DEFAULT_CREDS_PATHS) {
    try {
      if (fs.existsSync(p)) {
        return JSON.parse(fs.readFileSync(p, "utf8")) as RawCreds
      }
    } catch {
      // try the next candidate
    }
  }
  return null
}

export function wrapSecret(
  value: string | undefined
): { value: string } | undefined {
  return value ? { value } : undefined
}

// --- per-connector transforms (snake_case file → SDK connectorConfig shape) ---

export function transformStripeConfig(raw: any) {
  return { apiKey: wrapSecret(raw.api_key)! }
}

export function transformAdyenConfig(raw: any) {
  return {
    apiKey: wrapSecret(raw.api_key)!,
    merchantAccount: wrapSecret(raw.merchant_account)!,
  }
}

export function transformPaypalConfig(raw: any) {
  return {
    clientId: wrapSecret(raw.client_id)!,
    clientSecret: wrapSecret(raw.client_secret)!,
  }
}

export function transformGlobalpayConfig(raw: any) {
  return {
    appId: wrapSecret(raw.app_id)!,
    appKey: wrapSecret(raw.app_key)!,
  }
}

export function transformBraintreeConfig(raw: any) {
  return {
    publicKey: wrapSecret(raw.public_key)!,
    privateKey: wrapSecret(raw.private_key)!,
    merchantAccountId: wrapSecret(raw.metadata?.merchant_account_id),
    merchantConfigCurrency: raw.metadata?.merchant_config_currency,
    paypalClientId: raw.paypal_client_id,
  }
}

export function transformCybersourceConfig(raw: any, index = 0) {
  const entry = Array.isArray(raw) ? raw[index] : raw
  return {
    apiKey: wrapSecret(entry.api_key)!,
    merchantAccount: wrapSecret(entry.merchant_account)!,
    apiSecret: wrapSecret(entry.api_secret)!,
  }
}

export function transformMollieConfig(raw: any) {
  return {
    apiKey: wrapSecret(raw.api_key)!,
    profileToken: wrapSecret(raw.profile_token),
    baseUrl: raw.base_url,
    secondaryBaseUrl: raw.secondary_base_url,
  }
}

// PayU uses BodyKey auth: api_key (merchant key) + api_secret (merchant salt).
export function transformPayuConfig(raw: any) {
  return {
    apiKey: wrapSecret(raw.api_key)!,
    apiSecret: wrapSecret(raw.api_secret)!,
    ...(raw.base_url ? { baseUrl: raw.base_url } : {}),
  }
}

/**
 * Builds the connector→config map the app server expects (same shape as
 * app/run.ts loadCredentials()), including per-connector webhookSecret sourced
 * from the file (adyen.hmac_key/webhook_secret, paypal.webhook_id). Connectors
 * absent from the file are omitted, so the server skips them.
 */
export function buildServerCredentials(raw: RawCreds | null): Record<string, any> {
  const creds: Record<string, any> = {}
  if (!raw) return creds

  if (raw.stripe?.api_key) {
    creds.stripe = {
      ...transformStripeConfig(raw.stripe),
      // Publishable key the storefront reads from session data (not a secret).
      publishableKey: raw.stripe.publishable_key,
    }
  }

  if (raw.adyen?.api_key && raw.adyen?.merchant_account) {
    creds.adyen = {
      ...transformAdyenConfig(raw.adyen),
      webhookSecret: raw.adyen.hmac_key ?? raw.adyen.webhook_secret,
      // Adyen client key the drop-in reads from session data (not a secret).
      publishableKey: raw.adyen.client_key,
    }
  }

  if (raw.paypal?.client_id && raw.paypal?.client_secret) {
    creds.paypal = {
      ...transformPaypalConfig(raw.paypal),
      ...(raw.paypal.payer_id
        ? { payerId: wrapSecret(raw.paypal.payer_id) }
        : {}),
      includeShippingData: false,
      includeCustomerData: false,
      webhookSecret: raw.paypal.webhook_id ?? raw.paypal.webhook_secret,
    }
  }

  if (raw.braintree?.public_key && raw.braintree?.private_key) {
    creds.braintree = transformBraintreeConfig(raw.braintree)
  }

  if (raw.mollie?.api_key) {
    creds.mollie = transformMollieConfig(raw.mollie)
  }

  if (raw.payu?.api_key && raw.payu?.api_secret) {
    creds.payu = {
      ...transformPayuConfig(raw.payu),
      // URL the shopper returns to after the UPI/hosted-page redirect.
      returnUrl: raw.payu.return_url ?? "http://localhost:3000/return",
    }
  }

  const cybersource = Array.isArray(raw.cybersource)
    ? raw.cybersource[0]
    : raw.cybersource
  if (cybersource?.api_key && cybersource?.api_secret && cybersource?.merchant_account) {
    creds.cybersource = transformCybersourceConfig(raw.cybersource, 0)
  }

  if (raw.globalpay?.app_id && raw.globalpay?.app_key) {
    creds.globalpay = {
      ...transformGlobalpayConfig(raw.globalpay),
      returnUrl:
        raw.globalpay.return_url ?? "http://localhost:3000/return",
    }
  }

  return creds
}
