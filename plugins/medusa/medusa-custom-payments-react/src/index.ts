export type {
  PaymentConnector,
  InitiateParams,
  InitiateResult,
  ConfirmParams,
  ConfirmResult,
  ConnectorConfig,
} from "./types";

export { PaymentContainer } from "./containers/PaymentContainer";
export { connectors, getConnector } from "./connectors";

// Re-export connector implementations for direct use
export { adyenConnector } from "./connectors/adyen";
export { paypalConnector } from "./connectors/paypal";
export { globalpayConnector } from "./connectors/globalpay";

// Re-export connector React wrappers for direct use
export { AdyenWrapper } from "./connectors/adyen/AdyenWrapper";
export { StripeWrapper } from "./connectors/stripe/StripeWrapper";
export { PayPalWrapper } from "./connectors/paypal/PayPalWrapper";
export { GlobalPayWrapper } from "./connectors/globalpay/GlobalPayWrapper";
export { MollieWrapper } from "./connectors/mollie/MollieWrapper";

// Payment button components
export {
  PaymentButton,
  GlobalPayPaymentButton,
  StripePaymentButton,
  AdyenPaymentButton,
  PayPalPaymentButton,
  MolliePaymentButton,
  ManualTestPaymentButton,
} from "./components/payment-buttons";
export type {
  BasePaymentButtonProps,
  StripePaymentButtonProps,
  GlobalPayPaymentButtonProps,
  PayPalPaymentButtonProps,
  MolliePaymentButtonProps,
  PaymentButtonProps,
} from "./components/payment-buttons";

// Medusa storefront integration components
export { HyperswitchPrismConnectorPanel } from "./components/HyperswitchPrismConnectorPanel";
export { HyperswitchPrismPaymentButton } from "./components/HyperswitchPrismPaymentButton";
export { MollieReturnHandler } from "./components/MollieReturnHandler";
export type { MollieReturnHandlerProps } from "./components/MollieReturnHandler";

// Provider ID predicates and constants.
// NOTE: for Next.js SERVER components, import these from the server-safe subpath
// "@juspay-tech/medusa-custom-payments-react/predicates" instead of this barrel
// (the barrel pulls in "use client" components → createContext error).
export {
  isHyperswitchPrism,
  isHyperswitchPrismStripe,
  isHyperswitchPrismAdyen,
  isHyperswitchPrismPaypal,
  isHyperswitchPrismGlobalpay,
  isHyperswitchPrismMollie,
  isHyperswitchPrismPanel,
  HYPERSWITCH_PRISM_PROVIDER_IDS,
} from "./utils/predicates";

// Next.js redirect/notFound control-flow error detection (for re-throwing)
export { isNextControlFlowError } from "./utils/redirect-error";
