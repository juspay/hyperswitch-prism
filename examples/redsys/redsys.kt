// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py redsys
//
// Redsys — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="redsys processCheckoutCard"

package examples.redsys

import payments.PaymentMethodAuthenticationClient
import payments.PaymentClient
import payments.RefundClient
import payments.PaymentMethodAuthenticationServiceAuthenticateRequest
import payments.PaymentServiceCaptureRequest
import payments.PaymentServiceGetRequest
import payments.PaymentMethodAuthenticationServicePreAuthenticateRequest
import payments.PaymentServiceRefundRequest
import payments.RefundServiceGetRequest
import payments.PaymentServiceVoidRequest
import payments.Currency
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


// Flow: PaymentMethodAuthenticationService.Authenticate
fun authenticate(txnId: String) {
    val client = PaymentMethodAuthenticationClient(_defaultConfig)
    val request = PaymentMethodAuthenticationServiceAuthenticateRequest.newBuilder().apply {
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        paymentMethodBuilder.apply {  // Payment Method.
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
        authenticationDataBuilder.apply {  // Authentication Details.
            eci = "05"  // Electronic Commerce Indicator (ECI) from 3DS.
            cavv = "AAAAAAAAAA=="  // Cardholder Authentication Verification Value (CAVV).
            threedsServerTransactionId = "probe-3ds-txn-001"  // 3DS Server Transaction ID.
            messageVersion = "2.1.0"  // 3DS Message Version (e.g., "2.1.0", "2.2.0").
            dsTransactionId = "probe-ds-txn-001"  // Directory Server Transaction ID (DS Trans ID).
        }
        returnUrl = "https://example.com/3ds-return"  // URLs for Redirection.
        continueRedirectionUrl = "https://example.com/3ds-continue"
        browserInfoBuilder.apply {  // Contextual Information.
            colorDepth = 24  // Display Information.
            screenHeight = 900
            screenWidth = 1440
            javaEnabled = false  // Browser Settings.
            javaScriptEnabled = true
            language = "en-US"
            timeZoneOffsetMinutes = -480
            acceptHeader = "application/json"  // Browser Headers.
            userAgent = "Mozilla/5.0 (probe-bot)"
            acceptLanguage = "en-US,en;q=0.9"
            ipAddress = "1.2.3.4"  // Device Information.
        }
    }.build()
    val response = client.authenticate(request)
    println("Status: ${response.status.name}")
}

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

// Flow: PaymentMethodAuthenticationService.PreAuthenticate
fun preAuthenticate(txnId: String) {
    val client = PaymentMethodAuthenticationClient(_defaultConfig)
    val request = PaymentMethodAuthenticationServicePreAuthenticateRequest.newBuilder().apply {
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        paymentMethodBuilder.apply {  // Payment Method.
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
        enrolledFor3Ds = false  // Authentication Details.
        returnUrl = "https://example.com/3ds-return"  // URLs for Redirection.
    }.build()
    val response = client.pre_authenticate(request)
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
    val flow = args.firstOrNull() ?: "authenticate"
    when (flow) {
        "authenticate" -> authenticate(txnId)
        "capture" -> capture(txnId)
        "get" -> get(txnId)
        "preAuthenticate" -> preAuthenticate(txnId)
        "refund" -> refund(txnId)
        "refundGet" -> refundGet(txnId)
        "void" -> void(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: authenticate, capture, get, preAuthenticate, refund, refundGet, void")
    }
}
