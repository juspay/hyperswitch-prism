import { types, IntegrationError, ConnectorError } from "hyperswitch-prism"
import { PaymentSessionStatus } from "@medusajs/framework/utils"

const ZERO_DECIMAL_CURRENCIES = new Set([
  "jpy",
  "krw",
  "vnd",
  "gnf",
  "mga",
  "pyg",
  "rwf",
  "ugx",
  "xaf",
  "xof",
])

export function toMinorAmount(amount: number, currencyCode: string): number {
  const decimals = ZERO_DECIMAL_CURRENCIES.has(currencyCode.toLowerCase())
    ? 0
    : 2
  return Math.round(amount * Math.pow(10, decimals))
}

// Medusa amounts arrive as BigNumberInput: a number, a numeric string, a
// BigNumber instance ({ numeric }), or a raw value ({ value, precision }).
// Number() on the object forms yields NaN, which connectors reject.
export function normalizeAmount(amount: unknown): number {
  if (typeof amount === "number") return amount
  if (typeof amount === "string") return Number(amount)
  if (amount && typeof amount === "object") {
    const raw = amount as { numeric?: unknown; value?: unknown }
    if (raw.numeric !== undefined) return Number(raw.numeric)
    if (raw.value !== undefined) return Number(raw.value)
  }
  return Number(amount)
}

export function fromMinorAmount(
  minorAmount: number,
  currencyCode: string
): number {
  const decimals = ZERO_DECIMAL_CURRENCIES.has(currencyCode.toLowerCase())
    ? 0
    : 2
  return minorAmount / Math.pow(10, decimals)
}

export function getPermissions(
  connector: string,
  connectorConfig: Record<string, unknown>
): string[] | undefined {
  const config = connectorConfig as any
  if (config?.permissions && Array.isArray(config.permissions)) {
    return config.permissions
  }
  if (connector === "globalpay") {
    return ["PMT_POST_Create_Single"]
  }
  return undefined
}

export function getPaymentMethodType(
  connector: string,
  paymentMethodType?: types.PaymentMethodType
): types.PaymentMethodType | undefined {
  if (paymentMethodType !== undefined) {
    return paymentMethodType
  }
  if (connector === "braintree") {
    return types.PaymentMethodType.PAY_PAL
  }
  return undefined
}

export function extractValue(raw: any): string | null {
  if (!raw) return null
  if (typeof raw === "object" && "value" in raw) return (raw as any).value
  if (typeof raw === "string") return raw
  return null
}

export function toCurrency(currencyCode: string): types.Currency {
  const key = currencyCode.toUpperCase() as keyof typeof types.Currency
  return (types.Currency[key] ??
    types.Currency.CURRENCY_UNSPECIFIED) as unknown as types.Currency
}

export function mapPrismStatus(status: number): PaymentSessionStatus {
  const { PaymentStatus } = types
  switch (status) {
    case PaymentStatus.AUTHORIZED:
    case PaymentStatus.PARTIALLY_AUTHORIZED:
      return PaymentSessionStatus.AUTHORIZED
    case PaymentStatus.CHARGED:
    case PaymentStatus.PARTIAL_CHARGED:
    case PaymentStatus.PARTIAL_CHARGED_AND_CHARGEABLE:
      return PaymentSessionStatus.CAPTURED
    case PaymentStatus.AUTHENTICATION_PENDING:
    case PaymentStatus.CONFIRMATION_AWAITED:
    case PaymentStatus.PAYMENT_METHOD_AWAITED:
    case PaymentStatus.DEVICE_DATA_COLLECTION_PENDING:
      return PaymentSessionStatus.REQUIRES_MORE
    case PaymentStatus.VOIDED:
    case PaymentStatus.VOID_INITIATED:
    case PaymentStatus.VOIDED_POST_CAPTURE:
      return PaymentSessionStatus.CANCELED
    case PaymentStatus.AUTHORIZATION_FAILED:
    case PaymentStatus.AUTHENTICATION_FAILED:
    case PaymentStatus.CAPTURE_FAILED:
    case PaymentStatus.VOID_FAILED:
    case PaymentStatus.FAILURE:
    case PaymentStatus.ROUTER_DECLINED:
      return PaymentSessionStatus.ERROR
    default:
      return PaymentSessionStatus.PENDING
  }
}

export function mapRefundStatus(status: number): string {
  const { RefundStatus } = types
  switch (status) {
    case RefundStatus.REFUND_SUCCESS:
      return "refunded"
    case RefundStatus.REFUND_PENDING:
    case RefundStatus.REFUND_STATUS_UNSPECIFIED:
      return "pending"
    case RefundStatus.REFUND_FAILURE:
    case RefundStatus.REFUND_TRANSACTION_FAILURE:
      return "error"
    case RefundStatus.REFUND_MANUAL_REVIEW:
      return "requires_more"
    default:
      return "pending"
  }
}

export function buildError(message: string, error: unknown): Error {
  if (error instanceof IntegrationError) {
    return new Error(
      `[Hyperswitch Prism] ${message}: ${error.message} (code: ${error.errorCode})`
    )
  }
  if (error instanceof ConnectorError) {
    return new Error(
      `[Hyperswitch Prism] ${message}: ${error.message} (http: ${error.httpStatusCode})`
    )
  }
  return new Error(
    `[Hyperswitch Prism] ${message}: ${(error as Error).message}`
  )
}
