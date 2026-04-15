// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py easebuzz
//
// Easebuzz — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="easebuzz processCheckoutCard"

package examples.easebuzz

import payments.PaymentClient
import payments.RefundClient
import payments.PaymentServiceCaptureRequest
import payments.PaymentServiceCreateOrderRequest
import payments.PaymentServiceGetRequest
import payments.PaymentServiceRefundRequest
import payments.RefundServiceGetRequest
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

// Flow: PaymentService.CreateOrder
fun createOrder(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = PaymentServiceCreateOrderRequest.newBuilder().apply {
        merchantOrderId = "probe_order_001"  // Identification.
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    }.build()
    val response = client.create_order(request)
    println("Order: ${response.connectorOrderId}")
}

// Flow: PaymentService.Get
fun get(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildGetRequest("probe_connector_txn_001")
    val response = client.get(request)
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


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "capture"
    when (flow) {
        "capture" -> capture(txnId)
        "createOrder" -> createOrder(txnId)
        "get" -> get(txnId)
        "refund" -> refund(txnId)
        "refundGet" -> refundGet(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: capture, createOrder, get, refund, refundGet")
    }
}
