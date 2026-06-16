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
