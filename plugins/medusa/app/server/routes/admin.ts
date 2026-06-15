import { Router } from "express"
import { createPrismService, HyperswitchPrismOptions } from "../prism"
import { getSession, updateSession } from "../session-store"

export function createAdminRoutes(credentials?: Record<string, any>) {
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

  // Returns the stored payment session.
  router.get("/payments/:sessionId", (req, res) => {
    const { sessionId } = req.params
    const session = getSession(sessionId)
    if (!session) {
      return res.status(404).json({ error: `Payment session ${sessionId} not found` })
    }
    res.json({
      payment: {
        id: session.id,
        connector: session.connector,
        status: session.status,
        connectorTransactionId: session.connectorTransactionId,
        data: session.data,
      },
    })
  })

  // Captures an authorized payment.
  router.post("/payments/:sessionId/capture", async (req, res) => {
    const { sessionId } = req.params
    console.log("[POST /admin/payments/%s/capture]", sessionId)

    const session = getSession(sessionId)
    if (!session) {
      return res.status(404).json({ error: `Payment session ${sessionId} not found` })
    }

    const prismService = getPrismService(session.connector)
    if (!prismService) {
      return res.status(503).json({ error: `No credentials available for ${session.connector}` })
    }

    try {
      const result = await prismService.capturePayment({
        data: {
          connector: session.connector,
          ...session.data,
          ...(session.minorAmount !== undefined ? { minorAmount: session.minorAmount } : {}),
          ...(session.currency ? { currency: session.currency } : {}),
        },
        context: { idempotency_key: `capt_${Date.now()}` },
      })

      updateSession(sessionId, { status: "captured" })
      console.log("[POST /admin/payments/%s/capture] status=%s", sessionId, (result.data as any)?.captureStatus)

      res.json({ payment: { id: sessionId, status: "captured", data: result.data } })
    } catch (error) {
      console.error("[POST /admin/payments/%s/capture] Error: %s", sessionId, (error as Error).message)
      res.status(500).json({ error: (error as Error).message })
    }
  })

  // Refunds a captured payment.
  // Body: { amount }
  router.post("/payments/:sessionId/refund", async (req, res) => {
    const { sessionId } = req.params
    const { amount } = req.body
    console.log("[POST /admin/payments/%s/refund] amount=%s", sessionId, amount)

    const session = getSession(sessionId)
    if (!session) {
      return res.status(404).json({ error: `Payment session ${sessionId} not found` })
    }

    const prismService = getPrismService(session.connector)
    if (!prismService) {
      return res.status(503).json({ error: `No credentials available for ${session.connector}` })
    }

    // Use the connector transaction id (pspReference) if available — required for
    // connectors like Adyen where refund must reference the capture reference.
    const refundId = session.connectorTransactionId ?? (session.data.id as string | undefined)
    const refundData = refundId
      ? { ...session.data, id: refundId, connectorTransactionId: refundId }
      : session.data

    try {
      const result = await prismService.refundPayment({
        data: { connector: session.connector, ...refundData },
        amount,
        context: { idempotency_key: `ref_${Date.now()}` },
      })

      const refundStatus = (result.data as any)?.refundStatus
      console.log("[POST /admin/payments/%s/refund] refundStatus=%s", sessionId, refundStatus)

      res.json({
        refund: {
          id: `ref_${Date.now()}`,
          amount,
          status: refundStatus,
          data: result.data,
        },
      })
    } catch (error) {
      console.error("[POST /admin/payments/%s/refund] Error: %s", sessionId, (error as Error).message)
      res.status(500).json({ error: (error as Error).message })
    }
  })

  // Voids / cancels an authorized payment.
  router.post("/payments/:sessionId/cancel", async (req, res) => {
    const { sessionId } = req.params
    console.log("[POST /admin/payments/%s/cancel]", sessionId)

    const session = getSession(sessionId)
    if (!session) {
      return res.status(404).json({ error: `Payment session ${sessionId} not found` })
    }

    const prismService = getPrismService(session.connector)
    if (!prismService) {
      return res.status(503).json({ error: `No credentials available for ${session.connector}` })
    }

    try {
      const result = await prismService.cancelPayment({
        data: { connector: session.connector, ...session.data },
        context: {},
      })

      updateSession(sessionId, { status: "canceled" })
      console.log("[POST /admin/payments/%s/cancel]", sessionId)

      res.json({ payment: { id: sessionId, status: "canceled", data: result.data } })
    } catch (error) {
      console.error("[POST /admin/payments/%s/cancel] Error: %s", sessionId, (error as Error).message)
      res.status(500).json({ error: (error as Error).message })
    }
  })

  return router
}
