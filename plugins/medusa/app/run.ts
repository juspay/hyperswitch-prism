import "dotenv/config"
import { createApp } from "./server"
import { loadCredsFile, buildServerCredentials } from "./helpers/creds"

// Prefer creds.json (the repo's test-credentials convention) when present,
// falling back to environment variables otherwise.
function loadCredentials(): Record<string, any> {
  const fromFile = buildServerCredentials(loadCredsFile())
  if (Object.keys(fromFile).length > 0) {
    return fromFile
  }
  return loadCredentialsFromEnv()
}

function loadCredentialsFromEnv(): Record<string, any> {
  const creds: Record<string, any> = {}

  if (process.env.STRIPE_API_KEY) {
    creds.stripe = {
      apiKey: { value: process.env.STRIPE_API_KEY },
      publishableKey: process.env.STRIPE_PUBLISHABLE_KEY,
    }
  }

  if (process.env.ADYEN_API_KEY && process.env.ADYEN_MERCHANT_ACCOUNT) {
    creds.adyen = {
      apiKey: { value: process.env.ADYEN_API_KEY },
      merchantAccount: { value: process.env.ADYEN_MERCHANT_ACCOUNT },
      // Hex HMAC key from the Customer Area — mandatory for webhook handling
      webhookSecret: process.env.ADYEN_WEBHOOK_SECRET,
      // Adyen client key the drop-in reads from session data (not a secret).
      publishableKey: process.env.ADYEN_CLIENT_KEY,
    }
  }

  if (process.env.PAYPAL_CLIENT_ID && process.env.PAYPAL_CLIENT_SECRET) {
    creds.paypal = {
      clientId: { value: process.env.PAYPAL_CLIENT_ID },
      clientSecret: { value: process.env.PAYPAL_CLIENT_SECRET },
      ...(process.env.PAYPAL_PAYER_ID && {
        payerId: { value: process.env.PAYPAL_PAYER_ID },
      }),
      includeShippingData: false,
      includeCustomerData: false,
      // Webhook ID from the developer dashboard — mandatory for webhook handling
      webhookSecret: process.env.PAYPAL_WEBHOOK_ID,
    }
  }

  if (
    process.env.BRAINTREE_PUBLIC_KEY &&
    process.env.BRAINTREE_PRIVATE_KEY
  ) {
    creds.braintree = {
      publicKey: { value: process.env.BRAINTREE_PUBLIC_KEY },
      privateKey: { value: process.env.BRAINTREE_PRIVATE_KEY },
      ...(process.env.BRAINTREE_MERCHANT_ACCOUNT_ID && {
        merchantAccountId: { value: process.env.BRAINTREE_MERCHANT_ACCOUNT_ID },
      }),
    }
  }

  if (process.env.MOLLIE_API_KEY) {
    creds.mollie = { apiKey: { value: process.env.MOLLIE_API_KEY } }
  }

  if (
    process.env.CYBERSOURCE_API_KEY &&
    process.env.CYBERSOURCE_API_SECRET &&
    process.env.CYBERSOURCE_MERCHANT_ACCOUNT
  ) {
    creds.cybersource = {
      apiKey: { value: process.env.CYBERSOURCE_API_KEY },
      apiSecret: { value: process.env.CYBERSOURCE_API_SECRET },
      merchantAccount: { value: process.env.CYBERSOURCE_MERCHANT_ACCOUNT },
    }
  }

  if (process.env.GLOBALPAY_APP_ID && process.env.GLOBALPAY_APP_KEY) {
    creds.globalpay = {
      appId: { value: process.env.GLOBALPAY_APP_ID },
      appKey: { value: process.env.GLOBALPAY_APP_KEY },
      returnUrl:
        process.env.GLOBALPAY_RETURN_URL ?? "http://localhost:3000/return",
    }
  }

  return creds
}

const port = process.env.PORT ? parseInt(process.env.PORT, 10) : 3000
const credentials = loadCredentials()

const app = createApp(credentials)

const server = app.listen(port, () => {
  console.log(`[Server] Running on http://localhost:${port}`)
  console.log(`[Server] Health check: GET http://localhost:${port}/health`)

  const loaded = Object.keys(credentials)
  if (loaded.length > 0) {
    console.log(`[Server] Loaded credentials for: ${loaded.join(", ")}`)
  } else {
    console.log(
      `[Server] No credentials loaded. Payment routes will return 503.`
    )
    console.log(
      `[Server] Set env vars (e.g. STRIPE_API_KEY) to enable connectors.`
    )
  }

  console.log(`[Server] Routes:`)
  console.log(`  Store API:`)
  console.log(`    POST /store/payment-collections`)
  console.log(`    POST /store/payment-collections/:id/payment-sessions`)
  console.log(`    POST /store/payment-sessions/:id/reinitiate`)
  console.log(`    POST /store/carts/:cartId/complete`)
  console.log(`  Admin API:`)
  console.log(`    GET  /admin/payments/:id`)
  console.log(`    POST /admin/payments/:id/capture`)
  console.log(`    POST /admin/payments/:id/refund`)
  console.log(`    POST /admin/payments/:id/cancel`)
  console.log(`  Webhooks:`)
  console.log(`    POST /hooks/payment/hyperswitch-prism_hyperswitch-prism-<connector>`)
})

process.on("SIGTERM", () => {
  console.log("[Server] SIGTERM received, shutting down")
  server.close(() => process.exit(0))
})

process.on("SIGINT", () => {
  console.log("[Server] SIGINT received, shutting down")
  server.close(() => process.exit(0))
})
