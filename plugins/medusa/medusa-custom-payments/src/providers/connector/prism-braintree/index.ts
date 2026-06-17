import { InitiatePaymentOutput } from "@medusajs/framework/types"
import { PaymentSessionStatus } from "@medusajs/framework/utils"
import { InitiateConnectorContext } from "../types"

export async function initiatePayment({
  merchantClientSessionId,
  currencyCode,
  minorAmount,
  sessionData,
}: InitiateConnectorContext): Promise<InitiatePaymentOutput> {
  // Braintree returns generic wallet responses
  // (applePay, googlePay, paypal) based on payment method
  return {
    id: merchantClientSessionId,
    data: {
      id: merchantClientSessionId,
      currency: currencyCode,
      minorAmount,
      connector: "braintree",
      merchantClientSessionId,
      sessionData,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

// Braintree never obtains a real connector transaction id on this flow, so when a
// session is deleted (e.g. the shopper switches payment methods) there is nothing
// to void — skip it instead of letting the connector reject the bogus session id.
export function shouldSkipVoid(data: Record<string, unknown> | undefined): boolean {
  return !(data as any)?.connectorTransactionId
}
