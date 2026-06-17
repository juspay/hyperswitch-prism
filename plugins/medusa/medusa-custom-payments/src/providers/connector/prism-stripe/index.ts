import { InitiatePaymentOutput } from "@medusajs/framework/types"
import { PaymentSessionStatus } from "@medusajs/framework/utils"
import { extractValue } from "../../utils"
import { InitiateConnectorContext } from "../types"

export async function initiatePayment({
  options,
  merchantClientSessionId,
  currencyCode,
  minorAmount,
  sessionData,
  connectorSpecific,
}: InitiateConnectorContext): Promise<InitiatePaymentOutput> {
  const connectorData = connectorSpecific.stripe ?? connectorSpecific
  const clientSecret = extractValue(
    connectorData?.clientSecret ?? connectorData?.client_secret
  )

  // Publishable key from the provider config — surfaced in session data so the
  // storefront's Stripe Elements can initialise (it is not a secret).
  const publishableKey = extractValue(
    (options.connectorConfig as any)?.publishableKey
  )

  const connectorTransactionId =
    typeof clientSecret === "string"
      ? clientSecret.split("_secret_")[0]
      : merchantClientSessionId

  return {
    id: connectorTransactionId,
    data: {
      id: connectorTransactionId,
      client_secret: clientSecret,
      ...(publishableKey ? { publishableKey } : {}),
      currency: currencyCode,
      minorAmount,
      connector: "stripe",
      merchantClientSessionId,
      sessionData,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

// Stripe carries its real reference (the PaymentIntent id, `pi_…`) in `data.id`.
// When no client secret was obtained, `data.id` falls back to the Medusa session
// id — in that case there is no real PaymentIntent to void, so skip the void.
export function shouldSkipVoid(data: Record<string, unknown> | undefined): boolean {
  const d = data as any
  return !d?.id || d.id === d?.merchantClientSessionId
}
