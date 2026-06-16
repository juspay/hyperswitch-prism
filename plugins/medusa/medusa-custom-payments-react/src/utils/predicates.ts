// Matches both the canonical short form (pp_hyperswitch-prism_stripe) and the
// legacy long form (pp_hyperswitch-prism_hyperswitch-prism-stripe) produced by
// older medusa-config registrations that used id: "hyperswitch-prism-stripe".
const matchesPrismConnector = (id: string | undefined, connector: string) =>
  id === `pp_hyperswitch-prism_${connector}` ||
  id === `pp_hyperswitch-prism_hyperswitch-prism-${connector}`

export const isHyperswitchPrismStripe = (id?: string) =>
  matchesPrismConnector(id, "stripe")

export const isHyperswitchPrismAdyen = (id?: string) =>
  matchesPrismConnector(id, "adyen")

export const isHyperswitchPrismPaypal = (id?: string) =>
  matchesPrismConnector(id, "paypal")

export const isHyperswitchPrismGlobalpay = (id?: string) =>
  matchesPrismConnector(id, "globalpay")

export const isHyperswitchPrism = (id?: string) =>
  isHyperswitchPrismStripe(id) ||
  isHyperswitchPrismAdyen(id) ||
  isHyperswitchPrismPaypal(id) ||
  isHyperswitchPrismGlobalpay(id)

export const HYPERSWITCH_PRISM_PROVIDER_IDS = {
  stripe: "pp_hyperswitch-prism_stripe",
  adyen: "pp_hyperswitch-prism_adyen",
  paypal: "pp_hyperswitch-prism_paypal",
  globalpay: "pp_hyperswitch-prism_globalpay",
} as const
