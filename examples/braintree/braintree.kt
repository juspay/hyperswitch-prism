// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py braintree
//
// Braintree — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="braintree processCheckoutCard"

package examples.braintree

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.MerchantAuthenticationClient
import payments.PaymentMethodClient
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.BraintreeConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("capture", "create_client_authentication_token", "get", "refund", "tokenize", "void")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setBraintree(BraintreeConfig.newBuilder()
                .setPublicKey(SecretString.newBuilder().setValue("YOUR_PUBLIC_KEY").build())
                .setPrivateKey(SecretString.newBuilder().setValue("YOUR_PRIVATE_KEY").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setMerchantAccountId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ACCOUNT_ID").build())
                .setMerchantConfigCurrency("YOUR_MERCHANT_CONFIG_CURRENCY")
                .addAllApplePaySupportedNetworks(listOf("YOUR_APPLE_PAY_SUPPORTED_NETWORKS"))
                .addAllApplePayMerchantCapabilities(listOf("YOUR_APPLE_PAY_MERCHANT_CAPABILITIES"))
                .setApplePayLabel("YOUR_APPLE_PAY_LABEL")
                .setGpayMerchantName("YOUR_GPAY_MERCHANT_NAME")
                .setGpayMerchantId("YOUR_GPAY_MERCHANT_ID")
                .addAllGpayAllowedAuthMethods(listOf("YOUR_GPAY_ALLOWED_AUTH_METHODS"))
                .addAllGpayAllowedCardNetworks(listOf("YOUR_GPAY_ALLOWED_CARD_NETWORKS"))
                .setPaypalClientId("YOUR_PAYPAL_CLIENT_ID")
                .setGpayGatewayMerchantId("YOUR_GPAY_GATEWAY_MERCHANT_ID")
                .build())
            .build()
    )
    .build()



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
    }.build()
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

// Flow: MerchantAuthenticationService.CreateClientAuthenticationToken
fun createClientAuthenticationToken(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = MerchantAuthenticationClient(config)
    val request = MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest.newBuilder().apply {
        merchantClientSessionId = "probe_sdk_session_001"  // Infrastructure.
        paymentBuilder.apply {  // FrmClientAuthenticationContext frm = 5; // future: device fingerprinting PayoutClientAuthenticationContext payout = 6; // future: payout verification widget.
            amountBuilder.apply {
                minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
                currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
            }
        }
    }.build()
    val response = client.create_client_authentication_token(request)
    println("StatusCode: ${response.statusCode}")
}

// Flow: PaymentService.Get
fun get(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildGetRequest("probe_connector_txn_001")
    val response = client.get(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.Refund
fun refund(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildRefundRequest("probe_connector_txn_001")
    val response = client.refund(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Refund failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}

// Flow: PaymentMethodService.Tokenize
fun tokenize(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentMethodClient(config)
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
    }.build()
    val response = client.tokenize(request)
    println("Token: ${response.paymentMethodToken}")
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


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "capture"
    when (flow) {
        "capture" -> capture(txnId)
        "createClientAuthenticationToken" -> createClientAuthenticationToken(txnId)
        "get" -> get(txnId)
        "refund" -> refund(txnId)
        "tokenize" -> tokenize(txnId)
        "void" -> void(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: capture, createClientAuthenticationToken, get, refund, tokenize, void")
    }
}
