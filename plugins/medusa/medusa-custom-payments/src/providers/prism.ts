import {
  PaymentClient,
  MerchantAuthenticationClient,
  EventClient,
  ConnectorError,
  types,
} from "hyperswitch-prism"
import {
  AuthorizePaymentInput,
  AuthorizePaymentOutput,
  CancelPaymentInput,
  CancelPaymentOutput,
  CapturePaymentInput,
  CapturePaymentOutput,
  DeletePaymentInput,
  DeletePaymentOutput,
  GetPaymentStatusInput,
  GetPaymentStatusOutput,
  InitiatePaymentInput,
  InitiatePaymentOutput,
  ProviderWebhookPayload,
  RefundPaymentInput,
  RefundPaymentOutput,
  RetrievePaymentInput,
  RetrievePaymentOutput,
  UpdatePaymentInput,
  UpdatePaymentOutput,
  WebhookActionResult,
} from "@medusajs/framework/types"
import { PaymentActions, PaymentSessionStatus } from "@medusajs/framework/utils"
import { HyperswitchPrismOptions } from "./types"
import {
  toMinorAmount,
  fromMinorAmount,
  getPermissions,
  getPaymentMethodType,
  normalizeAmount,
  toCurrency,
  mapPrismStatus,
  mapRefundStatus,
  buildError,
  extractValue,
} from "./utils"
import { logger } from "./utils/logger"
import * as connectors from "./connector"

type PrismConfig = {
  connectorConfig: Record<string, unknown>
  options: { environment: types.Environment }
}

class PrismService {
  private paymentClient_: PaymentClient
  private authClient_: MerchantAuthenticationClient
  private eventClient_: EventClient
  private options_: HyperswitchPrismOptions
  private container_: Record<string, unknown>

  constructor(options: HyperswitchPrismOptions, container?: Record<string, unknown>) {
    this.options_ = options
    this.container_ = container || {}
    const prismConfig: PrismConfig = {
      connectorConfig: { [options.connector]: options.connectorConfig },
      options: {
        environment:
          options.environment === "PRODUCTION"
            ? types.Environment.PRODUCTION
            : types.Environment.SANDBOX,
      },
    }
    this.paymentClient_ = new PaymentClient(prismConfig as types.IConnectorConfig)
    this.authClient_ = new MerchantAuthenticationClient(
      prismConfig as types.IConnectorConfig
    )
    this.eventClient_ = new EventClient(prismConfig as types.IConnectorConfig)
  }

  private async getPaymentReferenceFromCart(resourceId: string): Promise<string | undefined> {
    if (!this.container_ || !resourceId) return undefined
    try {
      const cartService =
        (this.container_ as any).cartModuleService ||
        (this.container_ as any).cartService ||
        (this.container_ as any).cart
      if (!cartService || typeof cartService.retrieve !== "function") {
        return undefined
      }
      const cart = await cartService.retrieve(resourceId, { select: ["metadata"] })
      return cart?.metadata?.globalpay_payment_reference as string | undefined
    } catch (error) {
      logger.error("[PrismService.getPaymentReferenceFromCart] Could not read cart metadata: %s", (error as Error).message)
      return undefined
    }
  }

  buildError(message: string, error: unknown): Error {
    return buildError(message, error)
  }

  private getTransactionId(data: any): string | undefined {
    return data?.id
  }

  async initiatePayment({
    currency_code,
    amount,
    data,
    context,
  }: InitiatePaymentInput): Promise<InitiatePaymentOutput> {
    const merchantClientSessionId =
      (context?.idempotency_key as string) ??
      (data?.session_id as string) ??
      `hs_${Date.now()}`
    const minorAmount = toMinorAmount(normalizeAmount(amount), currency_code)

    const returnUrl =
      ((context as any)?.return_url as string) ??
      ((data as any)?.return_url as string)

    const paymentMethodTypeStr =
      ((data as any)?.payment_method_type as string) ??
      ((context as any)?.payment_method_type as string)

    const parsedPaymentMethodType = paymentMethodTypeStr
      ? types.PaymentMethodType[
          paymentMethodTypeStr as keyof typeof types.PaymentMethodType
        ]
      : undefined

    const paymentMethodType = getPaymentMethodType(
      this.options_.connector,
      parsedPaymentMethodType
    )

    const countryAlpha2CodeStr =
      ((data as any)?.country_alpha2_code as string) ??
      ((context as any)?.country_alpha2_code as string) ??
      ((context as any)?.country_code as string)

    const countryAlpha2Code = countryAlpha2CodeStr
      ? types.CountryAlpha2[
          countryAlpha2CodeStr as keyof typeof types.CountryAlpha2
        ]
      : undefined

    const permissions = getPermissions(
      this.options_.connector,
      this.options_.connectorConfig
    )

    // GlobalPay: re-initiation with card token — skip access-token fetch, just store the token
    if (this.options_.connector === "globalpay" && (data as any)?.paymentReference) {
      return connectors.globalpay.reInitiatePayment({
        data,
        merchantClientSessionId,
        currencyCode: currency_code,
        minorAmount,
      })
    }

    // Mollie Components: in-page card fields tokenize client-side. Skip the hosted
    // client-auth (which would create an orphan redirect payment). The first init
    // returns the public profileId for mollie.js; reinitiate stores the cardToken.
    // Klarna (PayLater redirect) has no client-side tokenization: the storefront
    // reinitiates with paymentMethodType="klarna" + billing, which is stored as-is
    // and consumed by authorizePayment (which builds the Klarna redirect payment).
    if (this.options_.connector === "mollie") {
      const isKlarnaReinit =
        String((data as any)?.paymentMethodType ?? "").toLowerCase() === "klarna"
      if ((data as any)?.cardToken || isKlarnaReinit) {
        return connectors.mollie.reInitiatePayment({
          data,
          merchantClientSessionId,
          currencyCode: currency_code,
          minorAmount,
        })
      }
      const profileId = extractValue(
        (this.options_.connectorConfig as any)?.profileToken
      )
      return {
        id: merchantClientSessionId,
        data: {
          id: merchantClientSessionId,
          profileId,
          currency: currency_code,
          minorAmount,
          connector: "mollie",
          merchantClientSessionId,
        },
        status: PaymentSessionStatus.PENDING,
      }
    }

    // Authorize.Net: raw-card flow — there is no SDK session
    // (createClientAuthenticationToken is not implemented for this connector).
    // First init returns a synthetic pending session so the card form can render;
    // re-initiation persists the card entered in the test client.
    if (this.options_.connector === "authorizedotnet") {
      if ((data as any)?.cardNumber) {
        return connectors.authorizedotnet.reInitiatePayment({
          data,
          merchantClientSessionId,
          currencyCode: currency_code,
          minorAmount,
        })
      }
      return connectors.authorizedotnet.initiatePayment({
        options: this.options_,
        merchantClientSessionId,
        currencyCode: currency_code,
        minorAmount,
        sessionData: {},
        connectorSpecific: {},
        authClient: this.authClient_,
      })
    }

    try {
      const req: any = {
        merchantClientSessionId,
        payment: {
          amount: {
            minorAmount,
            currency: toCurrency(currency_code),
          },
          ...(returnUrl ? { returnUrl } : {}),
          ...(paymentMethodType !== undefined
            ? { paymentMethodType }
            : {}),
          ...(countryAlpha2Code !== undefined
            ? { countryAlpha2Code }
            : {}),
        },
        ...(permissions ? { permissions: { values: permissions } } : {}),
      }

      const res = await this.authClient_.createClientAuthenticationToken(req)

      const statusCode = (res as any).statusCode as number | undefined
      if (statusCode !== undefined && (statusCode < 200 || statusCode >= 300)) {
        throw new Error(
          (res as any).error?.message ?? "createClientAuthenticationToken failed"
        )
      }

      const sessionData =
        (res as any).sessionData ?? (res as any).session_data ?? {}
      const connectorSpecific =
        sessionData?.connectorSpecific ?? sessionData?.connector_specific ?? {}

      const ctx: connectors.InitiateConnectorContext = {
        options: this.options_,
        merchantClientSessionId,
        currencyCode: currency_code,
        minorAmount,
        sessionData,
        connectorSpecific,
        authClient: this.authClient_,
      }

      switch (this.options_.connector) {
        case "stripe":
          return connectors.stripe.initiatePayment(ctx)
        case "adyen":
          return connectors.adyen.initiatePayment(ctx)
        case "paypal":
          return connectors.paypal.initiatePayment(ctx)
        case "braintree":
          return connectors.braintree.initiatePayment(ctx)
        case "globalpay":
          return connectors.globalpay.initiatePayment(ctx)
        case "cybersource":
          return connectors.cybersource.initiatePayment(ctx)
        // Mollie is handled by the Components short-circuit above (before the
        // hosted client-auth), so it never reaches this switch.
        default:
          // Fallback for any other connector — pass through raw session data
          return {
            id: merchantClientSessionId,
            data: {
              id: merchantClientSessionId,
              currency: currency_code,
              minorAmount,
              connector: this.options_.connector,
              merchantClientSessionId,
              sessionData,
            },
            status: PaymentSessionStatus.PENDING,
          }
      }
    } catch (error) {
      logger.error("[PrismService.initiatePayment] ERROR: %s", (error as Error).message)
      throw buildError("An error occurred in initiatePayment", error)
    }
  }

  async authorizePayment(
    input: AuthorizePaymentInput
  ): Promise<AuthorizePaymentOutput> {
    const connector = (input.data as any)?.connector as string | undefined

    if (connector === "adyen") {
      return connectors.adyen.authorizePayment(input, {
        getPaymentStatus: (i) => this.getPaymentStatus(i),
      })
    }

    if (connector === "globalpay") {
      return connectors.globalpay.authorizePayment(input, {
        options: this.options_,
        authClient: this.authClient_,
        paymentClient: this.paymentClient_,
        getPaymentReferenceFromCart: (resourceId) =>
          this.getPaymentReferenceFromCart(resourceId),
      })
    }

    if (connector === "paypal") {
      return connectors.paypal.authorizePayment(input, {
        options: this.options_,
        paymentClient: this.paymentClient_,
        authClient: this.authClient_,
      })
    }

    if (connector === "mollie") {
      return connectors.mollie.authorizePayment(input, {
        options: this.options_,
        paymentClient: this.paymentClient_,
        getPaymentStatus: (i) => this.getPaymentStatus(i),
      })
    }

    if (connector === "authorizedotnet") {
      return connectors.authorizedotnet.authorizePayment(input, {
        options: this.options_,
        paymentClient: this.paymentClient_,
      })
    }

    const result = await this.getPaymentStatus(input) as AuthorizePaymentOutput
    return result
  }

  async updatePayment(
    input: UpdatePaymentInput
  ): Promise<UpdatePaymentOutput> {
    return { data: input.data }
  }

  async getPaymentStatus({
    data,
  }: GetPaymentStatusInput): Promise<GetPaymentStatusOutput> {
    const connectorTransactionId = this.getTransactionId(data)
    if (!connectorTransactionId) {
      return { data, status: PaymentSessionStatus.PENDING }
    }

    const minorAmount = (data as any)?.minorAmount as number | undefined
    const currency = (data as any)?.currency as string | undefined
    const connector = (data as any)?.connector as string | undefined

    try {
      const res = await this.paymentClient_.get({
        connectorTransactionId,
        ...(minorAmount !== undefined && currency
          ? { amount: { minorAmount, currency: toCurrency(currency) } }
          : {}),
      })
      const status = (res as any).status as number | undefined
      const mappedStatus = mapPrismStatus(status ?? types.PaymentStatus.PAYMENT_STATUS_UNSPECIFIED)
      return {
        data: { ...(data as any), prismStatus: status, raw: res },
        status: mappedStatus,
      }
    } catch (error) {
      if (connector === "adyen" && connectors.adyen.isTransientStatusError(error)) {
        return { data, status: PaymentSessionStatus.PENDING }
      }
      logger.error("[PrismService.getPaymentStatus] ERROR: %s", (error as Error).message)
      throw buildError("An error occurred in getPaymentStatus", error)
    }
  }

  async capture({
    data,
    context,
  }: CapturePaymentInput): Promise<CapturePaymentOutput> {
    const connectorTransactionId = this.getTransactionId(data) as string
    // Medusa's CapturePaymentInput carries no amount — the provider captures
    // the full session amount. UCS rejects captures without amount_to_capture.
    const minorAmount = (data as any)?.minorAmount as number | undefined
    const currency = (data as any)?.currency as string | undefined

    try {
      const res = await this.paymentClient_.capture({
        merchantCaptureId:
          (context?.idempotency_key as string) ?? `capt_${Date.now()}`,
        connectorTransactionId,
        ...(minorAmount !== undefined && currency
          ? {
              amountToCapture: {
                minorAmount,
                currency: toCurrency(currency),
              },
            }
          : {}),
      })
      return {
        data: { ...(data as any), captureStatus: (res as any).status, raw: res },
      }
    } catch (error) {
      logger.error("[PrismService.capture] ERROR: %s", (error as Error).message)
      throw buildError("An error occurred in capturePayment", error)
    }
  }

  async refund({
    data,
    amount,
    context,
  }: RefundPaymentInput): Promise<RefundPaymentOutput> {
    if (this.options_.connector === "paypal") {
      return connectors.paypal.refundPayment(
        { data, amount, context },
        {
          options: this.options_,
          paymentClient: this.paymentClient_,
          authClient: this.authClient_,
        }
      )
    }

    if (this.options_.connector === "globalpay") {
      return connectors.globalpay.refundPayment(
        { data, amount, context },
        {
          options: this.options_,
          paymentClient: this.paymentClient_,
          authClient: this.authClient_,
        }
      )
    }

    if (this.options_.connector === "authorizedotnet") {
      return connectors.authorizedotnet.refundPayment(
        { data, amount, context },
        {
          options: this.options_,
          paymentClient: this.paymentClient_,
        }
      )
    }

    const connectorTransactionId = this.getTransactionId(data) as string
    const currency = (data as any)?.currency as string

    try {
      const res = await this.paymentClient_.refund({
        merchantRefundId:
          (context?.idempotency_key as string) ?? `ref_${Date.now()}`,
        connectorTransactionId,
        refundAmount: {
          minorAmount: toMinorAmount(normalizeAmount(amount), currency),
          currency: toCurrency(currency),
        },
      })

      const refundStatus = (res as any).status as number | undefined
      if (
        refundStatus === types.RefundStatus.REFUND_FAILURE ||
        refundStatus === types.RefundStatus.REFUND_TRANSACTION_FAILURE
      ) {
        throw new Error(
          (res as any).error?.message ?? "Refund failed at connector"
        )
      }

      return { data: { ...(data as any), refundStatus, refundStatusText: mapRefundStatus(refundStatus ?? 0), raw: res } }
    } catch (error) {
      logger.error("[PrismService.refund] ERROR: %s", (error as Error).message)
      throw buildError("An error occurred in refundPayment", error)
    }
  }

  async cancel({
    data,
    context,
  }: CancelPaymentInput): Promise<CancelPaymentOutput> {
    const connectorTransactionId = this.getTransactionId(data)
    if (!connectorTransactionId) {
      return { data }
    }

    // A session that was only initiated (never authorized) has no real connector
    // transaction to void. The "never authorized" marker differs per connector,
    // so each connector module owns its own shouldSkipVoid check. Skipping here
    // (rather than letting the void fail) keeps Medusa's delete-session flow —
    // run when a shopper switches payment methods — from aborting with a 500.
    const shouldSkipVoid: Record<
      string,
      (data: Record<string, unknown> | undefined) => boolean
    > = {
      paypal: connectors.paypal.shouldSkipVoid,
      adyen: connectors.adyen.shouldSkipVoid,
      braintree: connectors.braintree.shouldSkipVoid,
      cybersource: connectors.cybersource.shouldSkipVoid,
      globalpay: connectors.globalpay.shouldSkipVoid,
      stripe: connectors.stripe.shouldSkipVoid,
      mollie: connectors.mollie.shouldSkipVoid,
      authorizedotnet: connectors.authorizedotnet.shouldSkipVoid,
    }
    if (shouldSkipVoid[this.options_.connector]?.(data)) {
      return { data }
    }

    try {
      await this.paymentClient_.void({
        merchantVoidId:
          (context?.idempotency_key as string) ?? `void_${Date.now()}`,
        connectorTransactionId,
      })
      return { data }
    } catch (error) {
      if (error instanceof ConnectorError) {
        logger.error("[PrismService.cancel] ConnectorError (ignored): %s", (error as Error).message)
        return { data }
      }
      logger.error("[PrismService.cancel] ERROR: %s", (error as Error).message)
      throw buildError("An error occurred in cancelPayment", error)
    }
  }

  async deletePayment(
    input: DeletePaymentInput
  ): Promise<DeletePaymentOutput> {
    return { data: input.data }
  }

  async retrieve({
    data,
  }: RetrievePaymentInput): Promise<RetrievePaymentOutput> {
    const connectorTransactionId = this.getTransactionId(data) as string
    const currency = (data as any)?.currency as string
    const minorAmount = (data as any)?.minorAmount as number | undefined
    try {
      const res = await this.paymentClient_.get({
        connectorTransactionId,
        ...(minorAmount !== undefined && currency
          ? { amount: { minorAmount, currency: toCurrency(currency) } }
          : {}),
      })
      const rawAmount = (res as any)?.amount?.minorAmount as number | undefined
      if (rawAmount !== undefined && currency) {
        ; (res as any).amount = fromMinorAmount(rawAmount, currency)
      }
      return { data: { ...(data as any), raw: res } }
    } catch (error) {
      logger.error("[PrismService.retrieve] ERROR: %s", (error as Error).message)
      throw buildError("An error occurred in retrievePayment", error)
    }
  }

  async handleWebhook(
    webhookData: ProviderWebhookPayload["payload"]
  ): Promise<WebhookActionResult> {
    const deps = { options: this.options_, eventClient: this.eventClient_ }

    switch (this.options_.connector) {
      case "adyen":
        return connectors.adyen.handleWebhook(webhookData, deps)
      case "paypal":
        return connectors.paypal.handleWebhook(webhookData, deps)
      default:
        // stripe/globalpay/braintree/cybersource/mollie: UCS has no webhook
        // support (HandleEvent fails with WebhooksNotImplemented) — payment
        // state for these connectors is driven by the synchronous flows, so
        // acknowledge the event and ignore it instead of erroring into a
        // connector retry loop.
        return { action: PaymentActions.NOT_SUPPORTED }
    }
  }
}

export default PrismService
