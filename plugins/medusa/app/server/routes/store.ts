import { Router } from "express"
import { createPrismService, HyperswitchPrismOptions } from "../prism"
import {
  createCollection,
  createSession,
  getCollection,
  getSession,
  getSessionByCart,
  updateSession,
} from "../session-store"

// Strips provider id prefixes to get the raw connector name.
// Handles "pp_hyperswitch-prism_hyperswitch-prism-adyen", "hyperswitch-prism-adyen", "adyen".
function extractConnector(providerId: string): string {
  return providerId.split("-").at(-1) ?? providerId
}

export function createStoreRoutes(credentials?: Record<string, any>) {
  const router = Router()

  function getPrismService(connector: string) {
    if (!credentials?.[connector]) return null
    const options: HyperswitchPrismOptions = {
      connector: connector as any,
      connectorConfig: credentials[connector],
      environment: "SANDBOX",
    }
    return createPrismService(options)
  }

  // Creates an in-memory payment collection for a cart.
  // Body: { cart_id, currency_code, amount }
  router.post("/payment-collections", (req, res) => {
    const { cart_id, currency_code = "USD", amount = 0 } = req.body
    if (!cart_id) {
      return res.status(400).json({ error: "cart_id is required" })
    }
    const collection = createCollection(cart_id, currency_code, amount)
    console.log("[POST /store/payment-collections] id=%s cartId=%s", collection.id, cart_id)
    res.json({ payment_collection: { id: collection.id, cart_id: collection.cartId } })
  })

  // Initiates a payment session for a payment collection.
  // Body: { provider_id }
  router.post("/payment-collections/:collectionId/payment-sessions", async (req, res) => {
    const { collectionId } = req.params
    const { provider_id } = req.body
    if (!provider_id) {
      return res.status(400).json({ error: "provider_id is required" })
    }

    const collection = getCollection(collectionId)
    if (!collection) {
      return res.status(404).json({ error: `Payment collection ${collectionId} not found` })
    }

    const connector = extractConnector(provider_id)
    console.log(
      "[POST /store/payment-collections/%s/payment-sessions] provider_id=%s connector=%s",
      collectionId,
      provider_id,
      connector
    )

    const prismService = getPrismService(connector)
    if (!prismService) {
      return res.status(503).json({ error: `No credentials available for ${connector}` })
    }

    try {
      const result = await prismService.initiatePayment({
        currency_code: collection.currencyCode,
        amount: collection.amount,
        data: {},
        context: { cart_id: collection.cartId },
      })

      const resultData = result.data as Record<string, unknown>

      // The plugin's initiatePayment surfaces the connector's client-facing key
      // (Stripe publishable key / Adyen client key) as `data.publishableKey`,
      // sourced from connectorConfig.publishableKey — no harness-side injection.

      const session = createSession({
        collectionId,
        cartId: collection.cartId,
        connector,
        data: resultData,
        status: result.status as string,
        minorAmount: resultData.minorAmount as number | undefined,
        currency: (resultData.currency as string | undefined) ?? collection.currencyCode,
      })

      console.log(
        "[POST /store/payment-collections/%s/payment-sessions] sessionId=%s status=%s",
        collectionId,
        session.id,
        session.status
      )

      res.json({
        payment_collection: {
          id: collectionId,
          cart_id: collection.cartId,
          payment_sessions: [
            {
              id: session.id,
              provider_id,
              data: session.data,
              status: session.status,
            },
          ],
        },
      })
    } catch (error) {
      console.error(
        "[POST /store/payment-collections/%s/payment-sessions] Error: %s",
        collectionId,
        (error as Error).message
      )
      res.status(500).json({ error: (error as Error).message })
    }
  })

  // Completes a cart — triggers authorizePayment on the active session.
  router.post("/carts/:cartId/complete", async (req, res) => {
    const { cartId } = req.params
    console.log("[POST /store/carts/%s/complete]", cartId)

    const session = getSessionByCart(cartId)
    if (!session) {
      return res.status(404).json({ error: `No payment session found for cart ${cartId}` })
    }

    const prismService = getPrismService(session.connector)
    if (!prismService) {
      return res.status(503).json({ error: `No credentials available for ${session.connector}` })
    }

    try {
      const result = await prismService.authorizePayment({
        data: { connector: session.connector, ...session.data },
      })

      const resultData = result.data as Record<string, unknown>
      const connectorTransactionId =
        (resultData?.connectorTransactionId as string | undefined) ??
        (resultData?.id as string | undefined)

      updateSession(session.id, {
        status: result.status as string,
        connectorTransactionId,
        data: { ...session.data, ...resultData },
      })

      console.log(
        "[POST /store/carts/%s/complete] sessionId=%s status=%s",
        cartId,
        session.id,
        result.status
      )

      res.json({
        type: "order",
        order: {
          id: `order_${Date.now()}`,
          payment_status: result.status,
          payment_session_id: session.id,
        },
      })
    } catch (error) {
      console.error("[POST /store/carts/%s/complete] Error: %s", cartId, (error as Error).message)
      res.status(500).json({ error: (error as Error).message })
    }
  })

  // Re-initiates a payment session — mirrors what the storefront does in
  // HyperswitchPrismConnectorPanel.onInitiateSession after card tokenisation.
  // For GlobalPay, body.data must include { paymentReference, id }; the prism
  // service detects data.paymentReference and routes to reInitiatePayment(),
  // which stores the token in the session without making another network call.
  // Body: { data: Record<string, unknown> }
  router.post("/payment-sessions/:sessionId/reinitiate", async (req, res) => {
    const { sessionId } = req.params
    const existing = getSession(sessionId)
    if (!existing) {
      return res.status(404).json({ error: `Payment session ${sessionId} not found` })
    }
    const collection = getCollection(existing.collectionId)
    if (!collection) {
      return res.status(404).json({ error: `Payment collection ${existing.collectionId} not found` })
    }

    const prismService = getPrismService(existing.connector)
    if (!prismService) {
      return res.status(503).json({ error: `No credentials available for ${existing.connector}` })
    }

    try {
      const result = await prismService.initiatePayment({
        currency_code: collection.currencyCode,
        amount: collection.amount,
        data: { ...existing.data, ...(req.body.data ?? {}) },
        context: {},
      })
      const resultData = result.data as Record<string, unknown>
      const updated = updateSession(sessionId, {
        data: { ...existing.data, ...resultData },
      })
      console.log(
        "[POST /store/payment-sessions/%s/reinitiate] keys=%o",
        sessionId,
        Object.keys(resultData)
      )
      res.json({ payment_session: { id: updated!.id, data: updated!.data } })
    } catch (error) {
      console.error(
        "[POST /store/payment-sessions/%s/reinitiate] Error: %s",
        sessionId,
        (error as Error).message
      )
      res.status(500).json({ error: (error as Error).message })
    }
  })

  return router
}
