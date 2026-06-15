import { ConnectorError, IntegrationError } from "hyperswitch-prism"
import PrismService from "../../medusa-unified-payment/src/providers/prism"

import {
  loadCredsFile,
  transformStripeConfig,
  transformAdyenConfig,
  transformPaypalConfig,
  transformGlobalpayConfig,
  transformBraintreeConfig,
  transformCybersourceConfig,
  transformMollieConfig,
} from "../../app/helpers/creds"

// creds.json is the single source of truth; the shared loader and per-connector
// transforms live in app/helpers/creds.ts.
const loadCredentials = loadCredsFile

function logConnectorError(connector: string, error: unknown) {
  /* eslint-disable no-console */
  console.log(`\n--- ${connector} diagnostic ---`)
  if (error instanceof ConnectorError) {
    console.log("  type: ConnectorError")
    console.log(`  errorCode: ${error.errorCode}`)
    console.log(`  httpStatusCode: ${error.httpStatusCode}`)
    console.log(`  message: ${error.message}`)
    if (error.proto) {
      console.log(`  proto: ${JSON.stringify(error.proto, null, 2)}`)
    }
  } else if (error instanceof IntegrationError) {
    console.log("  type: IntegrationError")
    console.log(`  errorCode: ${error.errorCode}`)
    console.log(`  suggestedAction: ${error.suggestedAction}`)
    console.log(`  docUrl: ${error.docUrl}`)
    console.log(`  message: ${error.message}`)
    if (error.proto) {
      console.log(`  proto: ${JSON.stringify(error.proto, null, 2)}`)
    }
  } else if (error instanceof Error) {
    console.log("  type: Error")
    console.log(`  message: ${error.message}`)
  }
  console.log("---\n")
  /* eslint-enable no-console */
}

describe("initiatePayment integration — creds check", () => {
  const creds = loadCredentials()

  const itOrSkip = (name: string, fn: () => Promise<void>) => {
    if (creds) {
      it(name, fn)
    } else {
      it.skip(name, fn)
    }
  }

  const buildInput = (
    currency = "USD",
    amount = 1000,
    returnUrl = "https://example.com/return",
    extra: { payment_method_type?: string; country_alpha2_code?: string } = {}
  ) =>
  ({
    currency_code: currency,
    amount,
    data: { ...extra },
    context: { return_url: returnUrl },
  } as any)

  describe("stripe", () => {
    itOrSkip("returns session data with client_secret", async () => {
      const service = new PrismService({
        connector: "stripe",
        connectorConfig: transformStripeConfig(creds!.stripe),
        environment: "SANDBOX",
      })

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toBeDefined()
      expect(result.status).toBe("pending")
      expect(result.data).toBeDefined()
      expect(result.data!.connector).toBe("stripe")
      expect(result.data!.client_secret).toBeTruthy()
      expect(result.data!.currency).toBe("USD")
      expect(result.data!.minorAmount).toBe(100000)
    })
  })

  describe("adyen", () => {
    itOrSkip("returns session data with clientToken and publishableKey", async () => {
      const service = new PrismService({
        connector: "adyen",
        connectorConfig: transformAdyenConfig(creds!.adyen),
        environment: "SANDBOX",
      })

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toBeDefined()
      expect(result.status).toBe("pending")
      expect(result.data).toBeDefined()
      expect(result.data!.connector).toBe("adyen")
      expect(result.data!.clientToken).toBeTruthy()
      expect(result.data!.currency).toBe("USD")
      expect(result.data!.minorAmount).toBe(100000)
    })
  })

  describe("paypal", () => {
    itOrSkip("returns session data with clientToken", async () => {
      const service = new PrismService({
        connector: "paypal",
        connectorConfig: transformPaypalConfig(creds!.paypal),
        environment: "SANDBOX",
      })

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toBeDefined()
      expect(result.status).toBe("pending")
      expect(result.data).toBeDefined()
      expect(result.data!.connector).toBe("paypal")
      expect(
        result.data!.clientToken || result.data!.sessionToken
      ).toBeTruthy()
      expect(result.data!.currency).toBe("USD")
      expect(result.data!.minorAmount).toBe(100000)
    })
  })

  describe("globalpay", () => {
    itOrSkip("returns session data with accessToken", async () => {
      const service = new PrismService({
        connector: "globalpay",
        connectorConfig: transformGlobalpayConfig(creds!.globalpay),
        environment: "SANDBOX",
      })

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toBeDefined()
      expect(result.status).toBe("pending")
      expect(result.data).toBeDefined()
      expect(result.data!.connector).toBe("globalpay")
      expect(result.data!.accessToken).toBeTruthy()
      expect(result.data!.currency).toBe("USD")
      expect(result.data!.minorAmount).toBe(100000)
    })
  })

  describe("braintree", () => {
    itOrSkip(
      "returns session data with connector metadata",
      async () => {
        const service = new PrismService({
          connector: "braintree",
          connectorConfig: transformBraintreeConfig(creds!.braintree),
          environment: "SANDBOX",
        })

        try {
          const result = await service.initiatePayment(
            buildInput("USD", 1000, "https://example.com/return", {
              payment_method_type: "PAY_PAL",
              country_alpha2_code: "US",
            })
          )
          console.log(result)
          expect(result.id).toBeDefined()
          expect(result.status).toBe("pending")
          expect(result.data).toBeDefined()
          expect(result.data!.connector).toBe("braintree")
          expect(result.data!.sessionData).toBeDefined()
          expect(result.data!.currency).toBe("USD")
          expect(result.data!.minorAmount).toBe(100000)
        } catch (error) {
          console.log(error)
          logConnectorError("braintree", error)
          throw error
        }
      }
    )
  })

  describe("cybersource", () => {
    itOrSkip(
      "returns session data with captureContext",
      async () => {
        const service = new PrismService({
          connector: "cybersource",
          connectorConfig: transformCybersourceConfig(creds!.cybersource, 0),
          environment: "SANDBOX",
        })

        try {
          const result = await service.initiatePayment(
            buildInput(
              "USD",
              1000,
              "https://hyperswitch-demo-store.netlify.app/return"
            )
          )

          expect(result.id).toBeDefined()
          expect(result.status).toBe("pending")
          expect(result.data).toBeDefined()
          expect(result.data!.connector).toBe("cybersource")
          expect(result.data!.captureContext).toBeTruthy()
          expect(result.data!.currency).toBe("USD")
          expect(result.data!.minorAmount).toBe(100000)
        } catch (error) {
          logConnectorError("cybersource", error)
          throw error
        }
      }
    )
  })

  describe("mollie", () => {
    const mollieCreds = creds?.mollie
    const testFn = mollieCreds ? it : it.skip

    testFn(
      "returns session data with paymentId and checkoutUrl",
      async () => {
        const service = new PrismService({
          connector: "mollie",
          connectorConfig: transformMollieConfig(mollieCreds),
          environment: "SANDBOX",
        })

        try {
          const result = await service.initiatePayment(buildInput())

          expect(result.id).toBeDefined()
          expect(result.status).toBe("pending")
          expect(result.data).toBeDefined()
          expect(result.data!.connector).toBe("mollie")
          expect(result.data!.paymentId).toBeTruthy()
          expect(result.data!.checkoutUrl).toBeTruthy()
          expect(result.data!.currency).toBe("USD")
          expect(result.data!.minorAmount).toBe(100000)
        } catch (error) {
          logConnectorError("mollie", error)
          throw error
        }
      }
    )
  })
})
