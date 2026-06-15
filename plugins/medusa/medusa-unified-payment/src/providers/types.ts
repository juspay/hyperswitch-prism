export interface HyperswitchPrismOptions {
  connector:
    | "stripe"
    | "adyen"
    | "paypal"
    | "braintree"
    | "globalpay"
    | "cybersource"
    | "mollie"
  connectorConfig: Record<string, unknown>
  /**
   * Webhook verification secret — required to process webhooks; events that
   * cannot be source-verified are always rejected (dev and prod alike).
   * - adyen:  hex HMAC key generated in the Customer Area webhook settings
   * - paypal: webhook ID from the PayPal developer dashboard
   * - other connectors: unused (UCS has no webhook support for them)
   */
  webhookSecret?: string
  environment?: "SANDBOX" | "PRODUCTION"
  capture?: boolean
}

export interface HyperswitchPrismStripeOptions extends HyperswitchPrismOptions {
  connector: "stripe"
  connectorConfig: {
    apiKey: { value: string }
    /**
     * Publishable key (`pk_test_...` / `pk_live_...`). Not a secret — it is
     * surfaced in the payment session data as `publishableKey` so the
     * storefront's Stripe Elements can initialise without a separate env var.
     */
    publishableKey?: string | { value: string }
  }
}

export interface HyperswitchPrismAdyenOptions extends HyperswitchPrismOptions {
  connector: "adyen"
  connectorConfig: {
    apiKey: { value: string }
    merchantAccount: { value: string }
    /**
     * Adyen client key (`test_...` / `live_...`). Not a secret — it is surfaced
     * in the payment session data as `publishableKey` so the storefront's Adyen
     * drop-in can initialise without a separate env var.
     */
    publishableKey?: string | { value: string }
  }
}

export interface HyperswitchPrismPaypalOptions extends HyperswitchPrismOptions {
  connector: "paypal"
  connectorConfig: {
    clientId: { value: string }
    clientSecret: { value: string }
    payerId?: { value: string }
    /**
     * Send the shopper's shipping address to PayPal with the authorization
     * (PayPal uses SET_PROVIDED_ADDRESS). When false, PayPal collects the
     * address itself (GET_FROM_FILE). Defaults to false.
     */
    includeShippingData?: boolean
    /** Send customer details (name, email, phone) to PayPal. Defaults to false. */
    includeCustomerData?: boolean
  }
}

export interface HyperswitchPrismBraintreeOptions
  extends HyperswitchPrismOptions {
  connector: "braintree"
  connectorConfig: {
    publicKey: { value: string }
    privateKey: { value: string }
    merchantAccountId?: { value: string }
    merchantConfigCurrency?: string
    baseUrl?: string
    applePaySupportedNetworks?: string[]
    applePayMerchantCapabilities?: string[]
    applePayLabel?: string
    gpayMerchantName?: string
    gpayMerchantId?: string
    gpayAllowedAuthMethods?: string[]
    gpayAllowedCardNetworks?: string[]
    paypalClientId?: string
    gpayGatewayMerchantId?: string
  }
}

export interface HyperswitchPrismGlobalpayOptions
  extends HyperswitchPrismOptions {
  connector: "globalpay"
  connectorConfig: {
    appId: { value: string }
    appKey: { value: string }
    baseUrl?: string
    permissions?: string[]
    /** URL the shopper is redirected to after authorization (e.g. 3DS). */
    returnUrl?: string
  }
}

export interface HyperswitchPrismCybersourceOptions
  extends HyperswitchPrismOptions {
  connector: "cybersource"
  connectorConfig: {
    apiKey: { value: string }
    merchantAccount: { value: string }
    apiSecret: { value: string }
    baseUrl?: string
    disableAvs?: boolean
    disableCvn?: boolean
  }
}

export interface HyperswitchPrismMollieOptions
  extends HyperswitchPrismOptions {
  connector: "mollie"
  connectorConfig: {
    apiKey: { value: string }
    profileToken?: { value: string }
    baseUrl?: string
    secondaryBaseUrl?: string
  }
}
