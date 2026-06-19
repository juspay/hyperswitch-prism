import { MerchantAuthenticationClient } from "hyperswitch-prism"
import { HyperswitchPrismOptions } from "../types"

export type InitiateConnectorContext = {
  options: HyperswitchPrismOptions
  merchantClientSessionId: string
  currencyCode: string
  minorAmount: number
  sessionData: Record<string, any>
  connectorSpecific: Record<string, any>
  authClient: MerchantAuthenticationClient
}
