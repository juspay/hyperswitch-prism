import { test, expect, APIRequestContext } from "@playwright/test"
import { hasConnector } from "../fixtures/connectors"

// braintree/cybersource/mollie have no client checkout page, and their auth
// step needs a wallet nonce / transient token / hosted redirect that can't be
// completed headlessly. These specs exercise the plugin through the real HTTP
// routes at the initiate boundary and assert the connector-specific session
// shape, complementing the service-level Jest integration tests.

const CART = { cart_id: "cart_api_test", currency_code: "USD", amount: 100 }

async function initiate(
  request: APIRequestContext,
  connector: string
): Promise<{ status: string; data: Record<string, any> }> {
  const collRes = await request.post("/store/payment-collections", {
    data: CART,
  })
  expect(collRes.ok(), await collRes.text()).toBeTruthy()
  const collectionId = (await collRes.json()).payment_collection.id

  const sessRes = await request.post(
    `/store/payment-collections/${collectionId}/payment-sessions`,
    { data: { provider_id: connector } }
  )
  expect(sessRes.ok(), await sessRes.text()).toBeTruthy()
  const ps = (await sessRes.json()).payment_collection.payment_sessions[0]
  return { status: ps.status as string, data: ps.data as Record<string, any> }
}

interface ConnectorExpectation {
  connector: string
  // a session-data key whose presence proves initiate produced a usable session
  expectKey: string
}

const CASES: ConnectorExpectation[] = [
  { connector: "braintree", expectKey: "sessionData" },
  { connector: "cybersource", expectKey: "captureContext" },
  { connector: "mollie", expectKey: "checkoutUrl" },
  // PayU is an India-first UPI/redirect connector — its authorize step needs a
  // real UPI-app approval / hosted-page redirect that can't be driven headlessly,
  // so (like braintree/cybersource/mollie) we assert the initiate session shape.
  { connector: "payu", expectKey: "sessionData" },
]

for (const { connector, expectKey } of CASES) {
  test.describe(`${connector} (route-level)`, () => {
    test.skip(!hasConnector(connector), `${connector} credentials not present in creds.json`)

    test(`initiate returns a ${connector} session with ${expectKey}`, async ({ request }) => {
      const { status, data } = await initiate(request, connector)
      expect(status.toLowerCase()).toBe("pending")
      expect(data.connector).toBe(connector)
      expect(data[expectKey]).toBeTruthy()
      expect(data.currency).toBe("USD")
    })
  })
}
