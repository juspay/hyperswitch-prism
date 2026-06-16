import hyperswitchPrismProvider from "@juspay/medusa-custom-payments"
import type { HyperswitchPrismOptions } from "@juspay/medusa-custom-payments"

// The package's public entry exposes a Medusa ModuleProvider; the payment
// service class (which wraps PrismService) is its single registered service.
// PrismService itself is not reachable through the package's exports map.
const HyperswitchPrismService = (hyperswitchPrismProvider as any).services[0]

export type { HyperswitchPrismOptions }

export function createPrismService(options: HyperswitchPrismOptions) {
  return new HyperswitchPrismService({}, options)
}
