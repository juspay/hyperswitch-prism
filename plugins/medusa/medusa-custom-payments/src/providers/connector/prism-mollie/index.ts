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
