import type { PaymentConnector } from "../types";
import { adyenConnector } from "./adyen";
import { paypalConnector } from "./paypal";
import { globalpayConnector } from "./globalpay";

/**
 * Registry of all supported payment connectors.
 * Add new connectors here to make them available in the UI.
 *
 * Note: Stripe is intentionally not in this generic registry — it uses the
 * dedicated <StripeWrapper> (Medusa-style CardElement + confirmCardPayment)
 * rather than the generic PaymentConnector path.
 */
export const connectors: Record<string, PaymentConnector> = {
  adyen: adyenConnector,
  paypal: paypalConnector,
  globalpay: globalpayConnector,
};

/**
 * Retrieve a connector by name.
 * @throws if the connector is not registered.
 */
export function getConnector(name: string): PaymentConnector {
  const connector = connectors[name];
  if (!connector) {
    throw new Error(
      `Unsupported connector: "${name}". ` +
        `Available: ${Object.keys(connectors).join(", ")}`
    );
  }
  return connector;
}

/**
 * Register a custom connector at runtime.
 * Useful for adding connectors without modifying this package.
 */
export function registerConnector(
  name: string,
  connector: PaymentConnector
): void {
  connectors[name] = connector;
}
