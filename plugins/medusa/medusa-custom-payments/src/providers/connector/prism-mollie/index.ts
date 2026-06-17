import { InitiatePaymentOutput } from "@medusajs/framework/types"
import { PaymentSessionStatus } from "@medusajs/framework/utils"
import { extractValue } from "../../utils"
import { InitiateConnectorContext } from "../types"

export async function initiatePayment({
  merchantClientSessionId,
  currencyCode,
  minorAmount,
  sessionData,
  connectorSpecific,
}: InitiateConnectorContext): Promise<InitiatePaymentOutput> {
  const connectorData =
    connectorSpecific?.mollie ?? connectorSpecific
  const paymentId =
    connectorData?.paymentId ?? connectorData?.payment_id
  const checkoutUrl = extractValue(
    connectorData?.checkoutUrl ?? connectorData?.checkout_url
  )

  return {
    id: paymentId || merchantClientSessionId,
    data: {
      id: paymentId || merchantClientSessionId,
      paymentId,
      checkoutUrl,
      currency: currencyCode,
      minorAmount,
      connector: "mollie",
      merchantClientSessionId,
      sessionData,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

// Mollie carries its real reference (the payment id, `tr_…`) in `data.id`. When no
// payment id was obtained, `data.id` falls back to the Medusa session id — in that
// case there is no real payment to void, so skip the void.
export function shouldSkipVoid(data: Record<string, unknown> | undefined): boolean {
  const d = data as any
  return !d?.id || d.id === d?.merchantClientSessionId
}
