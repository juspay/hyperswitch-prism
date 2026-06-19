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
    connectorSpecific?.cybersource ?? connectorSpecific
  const captureContext = extractValue(
    connectorData?.captureContext ?? connectorData?.capture_context
  )
  const clientLibrary =
    connectorData?.clientLibrary ?? connectorData?.client_library
  const clientLibraryIntegrity =
    connectorData?.clientLibraryIntegrity ??
    connectorData?.client_library_integrity

  return {
    id: merchantClientSessionId,
    data: {
      id: merchantClientSessionId,
      captureContext,
      clientLibrary,
      clientLibraryIntegrity,
      currency: currencyCode,
      minorAmount,
      connector: "cybersource",
      merchantClientSessionId,
      sessionData,
    },
    status: PaymentSessionStatus.PENDING,
  }
}

// Cybersource never obtains a real connector transaction id on this flow, so when
// a session is deleted (e.g. the shopper switches payment methods) there is
// nothing to void — skip it instead of letting the connector reject the bogus
// session id.
export function shouldSkipVoid(data: Record<string, unknown> | undefined): boolean {
  return !(data as any)?.connectorTransactionId
}
