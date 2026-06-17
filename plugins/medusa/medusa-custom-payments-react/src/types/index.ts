/**
 * Core types shared across all payment connectors.
 */

export interface InitiateParams {
  /** Amount in major currency unit (e.g. dollars, euros) */
  amount: number;
  /** ISO 4217 currency code */
  currency_code: string;
  /** Medusa cart id */
  cartId: string;
  /** Additional context passed to the backend */
  context?: Record<string, unknown>;
}

export interface InitiateResult {
  /** Payment session status returned by the backend */
  status: "pending" | "requires_confirmation" | "succeeded";
  /** Connector-specific session data (clientToken, clientSecret, etc.) */
  data: unknown;
  /** Backend-generated payment session id */
  id?: string;
}

export interface ConfirmParams {
  /** Connector name */
  connector: string;
  /** Connector-specific payment data */
  data: unknown;
  /** Optional idempotency key */
  idempotencyKey?: string;
}

export interface ConfirmResult {
  status: "succeeded" | "failed" | "pending" | "captured";
  data?: unknown;
  error?: string;
}

export interface ConnectorConfig {
  /** Public key / client key for the connector SDK */
  publicKey?: string;
  /** Environment: sandbox or production */
  environment?: "test" | "live" | "sandbox" | "production";
  /** Country code for locale-specific behaviour */
  countryCode?: string;
}

/**
 * Every connector must implement this interface.
 * The PaymentContainer uses this to delegate connector-specific work.
 */
export interface PaymentConnector {
  /** Unique connector identifier */
  name: string;

  /**
   * Call the backend to create a payment session.
   * Your backend should forward this to PrismService.initiatePayment().
   */
  initiate: (params: InitiateParams) => Promise<InitiateResult>;

  /**
   * Render the payment UI into a DOM element.
   *
   * @param containerId – id of the HTML element to mount into
   * @param sessionData – value returned by `initiate()`
   * @param callbacks – success / error handlers
   */
  render: (
    containerId: string,
    sessionData: InitiateResult,
    callbacks: {
      onSubmit: (paymentData: unknown) => void;
      onError: (error: Error) => void;
    }
  ) => void;

  /**
   * Clean up SDK instances, event listeners, etc.
   */
  destroy: () => void;

  /**
   * Submit collected payment details to the backend for authorization.
   */
  confirm: (params: ConfirmParams) => Promise<ConfirmResult>;
}
