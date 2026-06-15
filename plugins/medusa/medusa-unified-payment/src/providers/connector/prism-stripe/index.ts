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
