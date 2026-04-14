// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py paysafe
//
// Paysafe — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="paysafe processCheckoutCard"

package examples.paysafe

import payments.PaymentClient
import payments.RefundClient
import payments.PaymentMethodClient
import payments.PaymentServiceCaptureRequest
import payments.PaymentServiceGetRequest
import payments.PaymentServiceProxySetupRecurringRequest
import payments.PaymentServiceRefundRequest
import payments.RefundServiceGetRequest
import payments.PaymentServiceSetupRecurringRequest
import payments.PaymentServiceTokenAuthorizeRequest
import payments.PaymentMethodServiceTokenizeRequest
import payments.PaymentServiceVoidRequest
import payments.AcceptanceType
import payments.AuthenticationType
import payments.CaptureMethod
import payments.Currency
import payments.FutureUsage
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment


private fun buildCaptureRequest(connectorTransactionIdStr: String): PaymentServiceCaptureRequest {
    return PaymentServiceCaptureRequest.newBuilder().apply {
        merchantCaptureId = "probe_capture_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        amountToCaptureBuilder.apply {  // Capture Details.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    }.build()
}

private fun buildGetRequest(connectorTransactionIdStr: String): PaymentServiceGetRequest {
    return PaymentServiceGetRequest.newBuilder().apply {
        merchantTransactionId = "probe_merchant_txn_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    }.build()
}

private fun buildRefundRequest(connectorTransactionIdStr: String): PaymentServiceRefundRequest {
    return PaymentServiceRefundRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        paymentAmount = 1000L  // Amount Information.
        refundAmountBuilder.apply {
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        reason = "customer_request"  // Reason for the refund.
    }.build()
}

private fun buildVoidRequest(connectorTransactionIdStr: String): PaymentServiceVoidRequest {
    return PaymentServiceVoidRequest.newBuilder().apply {
        merchantVoidId = "probe_void_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    }.build()
}

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your connector config here
    .build()


// Flow: PaymentService.Capture
fun capture(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildCaptureRequest("probe_connector_txn_001")
    val response = client.capture(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Capture failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}

// Flow: PaymentService.Get
fun get(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildGetRequest("probe_connector_txn_001")
    val response = client.get(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.ProxySetupRecurring
fun proxySetupRecurring(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = PaymentServiceProxySetupRecurringRequest.newBuilder().apply {
        merchantRecurringPaymentId = "probe_proxy_mandate_001"
        amountBuilder.apply {
            minorAmount = 0L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        cardProxyBuilder.apply {  // Card proxy for vault-aliased payments.
            cardNumberBuilder.value = "4111111111111111"  // Card Identification.
            cardExpMonthBuilder.value = "03"
            cardExpYearBuilder.value = "2030"
            cardCvcBuilder.value = "123"
            cardHolderNameBuilder.value = "John Doe"  // Cardholder Information.
        }
        addressBuilder.apply {
            billingAddressBuilder.apply {
            }
        }
        returnUrl = "https://example.com/return"
        customerAcceptanceBuilder.apply {
            acceptanceType = AcceptanceType.OFFLINE  // Type of acceptance (e.g., online, offline).
            acceptedAt = 0L  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        }
        authType = AuthenticationType.NO_THREE_DS
        setupFutureUsage = FutureUsage.OFF_SESSION
    }.build()
    val response = client.proxy_setup_recurring(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.Refund
fun refund(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildRefundRequest("probe_connector_txn_001")
    val response = client.refund(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Refund failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}

// Flow: RefundService.Get
fun refundGet(txnId: String) {
    val client = RefundClient(_defaultConfig)
    val request = RefundServiceGetRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"  // Identification.
        connectorTransactionId = "probe_connector_txn_001"
        refundId = "probe_refund_id_001"
    }.build()
    val response = client.refund_get(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.SetupRecurring
fun setupRecurring(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = PaymentServiceSetupRecurringRequest.newBuilder().apply {
        merchantRecurringPaymentId = "probe_mandate_001"  // Identification.
        amountBuilder.apply {  // Mandate Details.
            minorAmount = 0L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        paymentMethodBuilder.apply {
            cardBuilder.apply {  // Generic card payment.
                cardNumberBuilder.value = "4111111111111111"  // Card Identification.
                cardExpMonthBuilder.value = "03"
                cardExpYearBuilder.value = "2030"
                cardCvcBuilder.value = "737"
                cardHolderNameBuilder.value = "John Doe"  // Cardholder Information.
            }
        }
        addressBuilder.apply {  // Address Information.
            billingAddressBuilder.apply {
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Type of authentication to be used.
        enrolledFor3Ds = false  // Indicates if the customer is enrolled for 3D Secure.
        returnUrl = "https://example.com/mandate-return"  // URL to redirect after setup.
        setupFutureUsage = FutureUsage.OFF_SESSION  // Indicates future usage intention.
        requestIncrementalAuthorization = false  // Indicates if incremental authorization is requested.
        customerAcceptanceBuilder.apply {  // Details of customer acceptance.
            acceptanceType = AcceptanceType.OFFLINE  // Type of acceptance (e.g., online, offline).
            acceptedAt = 0L  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        }
    }.build()
    val response = client.setup_recurring(request)
    when (response.status.name) {
        "FAILED" -> throw RuntimeException("Setup failed: ${response.error.unifiedDetails.message}")
        else     -> println("Mandate stored: ${response.connectorRecurringPaymentId}")
    }
}

// Flow: PaymentService.TokenAuthorize
fun tokenAuthorize(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = PaymentServiceTokenAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_tokenized_txn_001"
        amountBuilder.apply {
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        connectorTokenBuilder.value = "pm_1AbcXyzStripeTestToken"  // Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
        addressBuilder.apply {
            billingAddressBuilder.apply {
            }
        }
        captureMethod = CaptureMethod.AUTOMATIC
        returnUrl = "https://example.com/return"
    }.build()
    val response = client.token_authorize(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentMethodService.Tokenize
fun tokenize(txnId: String) {
    val client = PaymentMethodClient(_defaultConfig)
    val request = PaymentMethodServiceTokenizeRequest.newBuilder().apply {
        amountBuilder.apply {  // Payment Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        paymentMethodBuilder.apply {
            cardBuilder.apply {  // Generic card payment.
                cardNumberBuilder.value = "4111111111111111"  // Card Identification.
                cardExpMonthBuilder.value = "03"
                cardExpYearBuilder.value = "2030"
                cardCvcBuilder.value = "737"
                cardHolderNameBuilder.value = "John Doe"  // Cardholder Information.
            }
        }
        addressBuilder.apply {  // Address Information.
            billingAddressBuilder.apply {
            }
        }
        returnUrl = "https://example.com/return"  // URLs for Redirection.
    }.build()
    val response = client.tokenize(request)
    println("Token: ${response.paymentMethodToken}")
}

// Flow: PaymentService.Void
fun void(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildVoidRequest("probe_connector_txn_001")
    val response = client.void(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Void failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "capture"
    when (flow) {
        "capture" -> capture(txnId)
        "get" -> get(txnId)
        "proxySetupRecurring" -> proxySetupRecurring(txnId)
        "refund" -> refund(txnId)
        "refundGet" -> refundGet(txnId)
        "setupRecurring" -> setupRecurring(txnId)
        "tokenAuthorize" -> tokenAuthorize(txnId)
        "tokenize" -> tokenize(txnId)
        "void" -> void(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: capture, get, proxySetupRecurring, refund, refundGet, setupRecurring, tokenAuthorize, tokenize, void")
    }
}
