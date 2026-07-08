import { Router } from "express"
import { createPrismService, HyperswitchPrismOptions } from "../prism"
import { getSession, updateSession } from "../session-store"

export function createWebhookRoutes(credentials?: Record<string, any>) {
  const router = Router()

  // POST /hooks/payment/:identifier
  // e.g. /hooks/payment/hyperswitch-prism_hyperswitch-prism-adyen
  // Connector is derived from the last hyphen-separated segment of the identifier.
  router.post("/:identifier", async (req, res) => {
    const { identifier } = req.params
    const connector = identifier.split("-").at(-1)
    console.log("[POST /hooks/payment/%s] connector=%s headers=%o", identifier, connector, Object.keys(req.headers))

    if (!connector || !credentials?.[connector]) {
      console.warn("[POST /hooks/payment/%s] No credentials for connector: %s", identifier, connector)
      return res.status(503).json({ error: `No credentials available for ${connector}` })
    }

    // Source verification is mandatory — there is no unverified path, even in
    // dev. Adyen needs the hex HMAC key, PayPal the webhook ID.
    const needsWebhookSecret = connector === "adyen" || connector === "paypal"
    if (needsWebhookSecret && !credentials[connector].webhookSecret) {
      console.error(
        "[POST /hooks/payment/%s] webhook secret not configured for %s — verification is mandatory (set %s)",
        identifier,
        connector,
        connector === "adyen" ? "ADYEN_WEBHOOK_SECRET" : "PAYPAL_WEBHOOK_ID"
      )
      return res.status(503).json({
        error: `Webhook secret not configured for ${connector} — verification is mandatory`,
      })
    }

    const options: HyperswitchPrismOptions = {
      connector: connector as any,
      connectorConfig: credentials[connector],
      environment: "SANDBOX",
      webhookSecret: credentials[connector].webhookSecret,
    }
    const prismService = createPrismService(options)

    try {
      const result = await prismService.getWebhookActionAndData({
        rawData: req.body,
        headers: req.headers as Record<string, string>,
      })
      console.log("[POST /hooks/payment/%s] action=%s data=%o", identifier, result.action, result.data)

      // Update stored session if we can correlate the webhook to one
      const webhookData = result.data as Record<string, unknown> | undefined
      const sessionId = webhookData?.session_id as string | undefined
      const connectorTransactionId = webhookData?.connector_transaction_id as string | undefined
      if (sessionId) {
        const session = getSession(sessionId)
        if (session) {
          updateSession(sessionId, {
            connectorTransactionId: connectorTransactionId ?? session.connectorTransactionId,
          })
        }
      }

      res.json(result)
    } catch (error) {
      console.error("[POST /hooks/payment/%s] Error: %s", identifier, (error as Error).message)
      res.status(500).json({ error: (error as Error).message })
    }
  })

  return router
}
