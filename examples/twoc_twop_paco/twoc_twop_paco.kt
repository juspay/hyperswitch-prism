// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py twoc_twop_paco
//
// Twoc_Twop_Paco — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="twoc_twop_paco processCheckoutCard"

package examples.twoc_twop_paco

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.RefundClient
import payments.PaymentMethodAuthenticationClient
import payments.AuthenticationType
import payments.CaptureMethod
import payments.CountryAlpha2
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.TwocTwopPacoConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("authorize", "get", "capture", "void", "reverse", "refund", "refund_get", "post_authenticate")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setTwocTwopPaco(TwocTwopPacoConfig.newBuilder()
                .setAccessToken(SecretString.newBuilder().setValue("YOUR_ACCESS_TOKEN").build())
                .setOfficeId(SecretString.newBuilder().setValue("YOUR_OFFICE_ID").build())
                .setPacoKid(SecretString.newBuilder().setValue("YOUR_PACO_KID").build())
                .setMerchantSigningPrivateKey(SecretString.newBuilder().setValue("YOUR_MERCHANT_SIGNING_PRIVATE_KEY").build())
                .setMerchantEncryptionPrivateKey(SecretString.newBuilder().setValue("YOUR_MERCHANT_ENCRYPTION_PRIVATE_KEY").build())
                .setPacoSigningPublicKey(SecretString.newBuilder().setValue("YOUR_PACO_SIGNING_PUBLIC_KEY").build())
                .setPacoEncryptionPublicKey(SecretString.newBuilder().setValue("YOUR_PACO_ENCRYPTION_PUBLIC_KEY").build())
                .setResponseAudience(SecretString.newBuilder().setValue("YOUR_RESPONSE_AUDIENCE").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()



private fun buildAuthorizeRequest(captureMethodStr: String): PaymentServiceAuthorizeRequest {
    return PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"  // Identification.
        amountBuilder.apply {  // The amount for the payment.
            minorAmount = 10000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.PHP  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        paymentMethodBuilder.apply {  // Payment method to be used.
            cardBuilder.apply {  // Generic card payment.
                cardNumberBuilder.apply {  // Card Identification.
                    value = "4111111111111111"
                }
                cardExpMonthBuilder.apply {
                    value = "12"
                }
                cardExpYearBuilder.apply {
                    value = "2027"
                }
                cardCvcBuilder.apply {
                    value = "123"
                }
                cardHolderNameBuilder.apply {  // Cardholder Information.
                    value = "Test Customer"
                }
                cardType = "credit"
            }
        }
        addressBuilder.apply {  // Address Information.
            billingAddressBuilder.apply {
                countryAlpha2Code = CountryAlpha2.PH
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Authentication Details.
        returnUrl = "https://example.com/return"  // URLs for Redirection and Webhooks.
        webhookUrl = "https://example.com/webhook"
    }.build()
}

private fun buildGetRequest(connectorTransactionIdStr: String): PaymentServiceGetRequest {
    return PaymentServiceGetRequest.newBuilder().apply {

    }.build()
}

private fun buildCaptureRequest(connectorTransactionIdStr: String): PaymentServiceCaptureRequest {
    return PaymentServiceCaptureRequest.newBuilder().apply {

    }.build()
}

private fun buildVoidRequest(connectorTransactionIdStr: String): PaymentServiceVoidRequest {
    return PaymentServiceVoidRequest.newBuilder().apply {

    }.build()
}

private fun buildReverseRequest(connectorTransactionIdStr: String): PaymentServiceReverseRequest {
    return PaymentServiceReverseRequest.newBuilder().apply {

    }.build()
}

private fun buildRefundRequest(connectorTransactionIdStr: String): PaymentServiceRefundRequest {
    return PaymentServiceRefundRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        paymentAmount = 10000L  // Amount Information.
        refundAmountBuilder.apply {
            minorAmount = 10000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.PHP  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        reason = "customer request"  // Reason for the refund.
        refundMetadataBuilder.apply {  // Metadata specific to the refund.
            value = "{\"original_order_no\":\"probe_txn_001\"}"
        }
    }.build()
}

// Scenario: One-step Payment (Authorize + Capture)
// Simple payment that authorizes and captures in one call. Use for immediate charges.
fun processCheckoutAutocapture(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val authorizeResponse = paymentClient.authorize(buildAuthorizeRequest("AUTOMATIC"))

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    return mapOf("status" to authorizeResponse.status.name, "transactionId" to authorizeResponse.connectorTransactionId, "error" to authorizeResponse.error)
}

// Scenario: Card Payment (Authorize + Capture)
// Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
fun processCheckoutCard(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val authorizeResponse = paymentClient.authorize(buildAuthorizeRequest("MANUAL"))

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    // Step 2: Capture — settle the reserved funds
    val captureResponse = paymentClient.capture(buildCaptureRequest(authorizeResponse.connectorTransactionId ?: ""))

    if (captureResponse.status.name == "FAILED")
        throw RuntimeException("Capture failed: ${captureResponse.error.unifiedDetails.message}")

    return mapOf("status" to captureResponse.status.name, "transactionId" to authorizeResponse.connectorTransactionId, "error" to authorizeResponse.error)
}

// Scenario: Refund
// Return funds to the customer for a completed payment.
fun processRefund(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val authorizeResponse = paymentClient.authorize(buildAuthorizeRequest("AUTOMATIC"))

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    // Step 2: Refund — return funds to the customer
    val refundResponse = paymentClient.refund(buildRefundRequest(authorizeResponse.connectorTransactionId ?: ""))

    if (refundResponse.status.name == "FAILED")
        throw RuntimeException("Refund failed: ${refundResponse.error.unifiedDetails.message}")

    return mapOf("status" to refundResponse.status.name, "error" to refundResponse.error)
}

// Flow: PaymentService.Authorize (Card)
fun authorize(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildAuthorizeRequest("AUTOMATIC")
    val response = client.authorize(request)
    when (response.status.name) {
        "FAILED"  -> throw RuntimeException("Authorize failed: ${response.error.unifiedDetails.message}")
        "PENDING" -> println("Pending — await webhook before proceeding")
        else      -> println("Authorized: ${response.connectorTransactionId}")
    }
}

// Flow: PaymentService.Get
fun get(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildGetRequest("probe_connector_txn_001")
    val response = client.get(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.Capture
fun capture(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildCaptureRequest("probe_connector_txn_001")
    val response = client.capture(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Capture failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}

// Flow: PaymentService.Void
fun void(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildVoidRequest("probe_connector_txn_001")
    val response = client.void(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Void failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}

// Flow: PaymentService.Reverse
fun reverse(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildReverseRequest("probe_connector_txn_001")
    val response = client.reverse(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.Refund
fun refund(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildRefundRequest("CEBU0000000000000")
    val response = client.refund(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Refund failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}

// Flow: RefundService.Get
fun refundGet(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = RefundClient(config)
    val request = RefundServiceGetRequest.newBuilder().apply {

    }.build()
    val response = client.refund_get(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentMethodAuthenticationService.PostAuthenticate
fun postAuthenticate(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentMethodAuthenticationClient(config)
    val request = PaymentMethodAuthenticationServicePostAuthenticateRequest.newBuilder().apply {

    }.build()
    val response = client.post_authenticate(request)
    println("Status: ${response.status.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "processCheckoutAutocapture"
    when (flow) {
        "processCheckoutAutocapture" -> processCheckoutAutocapture(txnId)
        "processCheckoutCard" -> processCheckoutCard(txnId)
        "processRefund" -> processRefund(txnId)
        "authorize" -> authorize(txnId)
        "get" -> get(txnId)
        "capture" -> capture(txnId)
        "void" -> void(txnId)
        "reverse" -> reverse(txnId)
        "refund" -> refund(txnId)
        "refundGet" -> refundGet(txnId)
        "postAuthenticate" -> postAuthenticate(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: processCheckoutAutocapture, processCheckoutCard, processRefund, authorize, get, capture, void, reverse, refund, refundGet, postAuthenticate")
    }
}
