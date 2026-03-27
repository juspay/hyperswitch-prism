/**
 * Re-exports for payment method types.
 *
 * Usage:
 *   import payments.PaymentMethod
 *   import payments.CardDetails
 *   import payments.CardNetwork
 *
 * Mirrors the JavaScript `payment_methods` namespace and Python `PaymentMethodsNamespace`.
 */
@file:Suppress("unused")

package payments

typealias PaymentMethod = types.PaymentMethods.PaymentMethod
typealias CardDetails = types.PaymentMethods.CardDetails
typealias CardNumberType = types.PaymentMethods.CardNumberType
typealias NetworkTokenType = types.PaymentMethods.NetworkTokenType
typealias CardRedirect = types.PaymentMethods.CardRedirect
typealias CardNetwork = types.PaymentMethods.CardNetwork
typealias TokenPaymentMethodType = types.PaymentMethods.TokenPaymentMethodType
typealias CountryAlpha2 = types.PaymentMethods.CountryAlpha2
