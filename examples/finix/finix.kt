// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py finix
//
// Finix — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="finix processCheckoutCard"

package examples.finix

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.CustomerClient
import payments.RefundClient
import payments.PaymentMethodClient
import payments.CaptureMethod
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment


val SUPPORTED_FLOWS = listOf<String>("capture", "create_customer", "get", "refund", "refund_get", "token_authorize", "tokenize", "void")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Finix credentials here
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

// Flow: CustomerService.Create
fun createCustomer(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = CustomerClient(config)
    val request = CustomerServiceCreateRequest.newBuilder().apply {
        merchantCustomerId = "cust_probe_123"  // Identification.
        customerName = "John Doe"  // Name of the customer.
        emailBuilder.value = "test@example.com"  // Email address of the customer.
        phoneNumber = "4155552671"  // Phone number of the customer.
    }.build()
    val response = client.create(request)
    println("Customer: ${response.connectorCustomerId}")
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

// Flow: RefundService.Get
fun refundGet(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = RefundClient(config)
    val request = RefundServiceGetRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"  // Identification.
        connectorTransactionId = "probe_connector_txn_001"
        refundId = "probe_refund_id_001"
    }.build()
    val response = client.refund_get(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.TokenAuthorize
fun tokenAuthorize(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
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
        customerBuilder.apply {  // Customer Information.
            id = "cust_probe_123"  // Internal customer ID.
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
        "createCustomer" -> createCustomer(txnId)
        "get" -> get(txnId)
        "refund" -> refund(txnId)
        "refundGet" -> refundGet(txnId)
        "tokenAuthorize" -> tokenAuthorize(txnId)
        "tokenize" -> tokenize(txnId)
        "void" -> void(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: capture, createCustomer, get, refund, refundGet, tokenAuthorize, tokenize, void")
    }
}
