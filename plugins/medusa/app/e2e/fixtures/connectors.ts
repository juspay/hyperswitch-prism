import { loadCredsFile, RawCreds } from "../../helpers/creds"

// The raw creds.json, loaded once. Specs use this to skip connectors that have
// no credentials and to read connector-specific test material (Adyen HMAC key,
// PayPal sandbox buyer).
export const rawCreds: RawCreds | null = loadCredsFile()

export function hasConnector(name: string): boolean {
  const c = rawCreds?.[name]
  if (!c) return false
  // cybersource may be an array of accounts
  const entry = Array.isArray(c) ? c[0] : c
  return Boolean(entry && Object.keys(entry).length > 0)
}

export function adyenHmacKey(): string | undefined {
  return rawCreds?.adyen?.hmac_key ?? rawCreds?.adyen?.webhook_secret
}

// True only when a real hex HMAC key is configured (not the example placeholder).
export function adyenHmacKeyReady(): boolean {
  const k = adyenHmacKey()
  return Boolean(k) && !/REPLACE_WITH/i.test(k as string)
}

export function adyenMerchantAccount(): string | undefined {
  return rawCreds?.adyen?.merchant_account
}

export function paypalSandboxBuyer(): { email: string; password: string } | undefined {
  return rawCreds?.paypal?.sandbox_buyer
}
