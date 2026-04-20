// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py bamboraapac
//
// Bamboraapac — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="bamboraapac processCheckoutCard"

package examples.bamboraapac

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment


val SUPPORTED_FLOWS = listOf<String>("authorize", "capture", "get", "proxy_authorize", "proxy_setup_recurring", "recurring_charge", "refund", "refund_get", "setup_recurring")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Bamboraapac credentials here
    .build()


// Scenario: One-step Payment (Authorize + Capture)
// Simple payment that authorizes and captures in one call. Use for immediate charges.
fun processCheckoutAutocapture(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val authorizeResponse = paymentClient.authorize(.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"
        minorAmount = 1000L
        currency = "USD"
        cardNumber = "4111111111111111"
        cardExpMonth = "03"
        cardExpYear = "2030"
        cardCvc = "737"
        cardHolderName = "John Doe"
        captureMethod = "AUTOMATIC"
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
    }.build())

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
    val authorizeResponse = paymentClient.authorize(.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"
        minorAmount = 1000L
        currency = "USD"
        cardNumber = "4111111111111111"
        cardExpMonth = "03"
        cardExpYear = "2030"
        cardCvc = "737"
        cardHolderName = "John Doe"
        captureMethod = "MANUAL"
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
    }.build())

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    // Step 2: Capture — settle the reserved funds
    val captureResponse = paymentClient.capture(.newBuilder().apply {
        merchantCaptureId = "probe_capture_001"
        minorAmount = 1000L
        currency = "USD"
        connectorTransactionId = authorizeResponse.connectorTransactionId  // from Authorize
    }.build())

    if (captureResponse.status.name == "FAILED")
        throw RuntimeException("Capture failed: ${captureResponse.error.unifiedDetails.message}")

    return mapOf("status" to captureResponse.status.name, "transactionId" to authorizeResponse.connectorTransactionId, "error" to authorizeResponse.error)
}

// Scenario: Refund
// Return funds to the customer for a completed payment.
fun processRefund(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val authorizeResponse = paymentClient.authorize(.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"
        minorAmount = 1000L
        currency = "USD"
        cardNumber = "4111111111111111"
        cardExpMonth = "03"
        cardExpYear = "2030"
        cardCvc = "737"
        cardHolderName = "John Doe"
        captureMethod = "AUTOMATIC"
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
    }.build())

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    // Step 2: Refund — return funds to the customer
    val refundResponse = paymentClient.refund(.newBuilder().apply {
        merchantRefundId = "probe_refund_001"
        paymentAmount = 1000L
        minorAmount = 1000L
        currency = "USD"
        reason = "customer_request"
        connectorTransactionId = authorizeResponse.connectorTransactionId  // from Authorize
    }.build())

    if (refundResponse.status.name == "FAILED")
        throw RuntimeException("Refund failed: ${refundResponse.error.unifiedDetails.message}")

    return mapOf("status" to refundResponse.status.name, "error" to refundResponse.error)
}

// Scenario: Get Payment Status
// Retrieve current payment status from the connector.
fun processGetPayment(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val authorizeResponse = paymentClient.authorize(.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"
        minorAmount = 1000L
        currency = "USD"
        cardNumber = "4111111111111111"
        cardExpMonth = "03"
        cardExpYear = "2030"
        cardCvc = "737"
        cardHolderName = "John Doe"
        captureMethod = "MANUAL"
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
    }.build())

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    // Step 2: Get — retrieve current payment status from the connector
    val getResponse = paymentClient.get(.newBuilder().apply {
        merchantTransactionId = "probe_merchant_txn_001"
        minorAmount = 1000L
        currency = "USD"
        connectorTransactionId = authorizeResponse.connectorTransactionId  // from Authorize
    }.build())

    return mapOf("status" to getResponse.status.name, "transactionId" to getResponse.connectorTransactionId, "error" to getResponse.error)
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "processCheckoutAutocapture"
    when (flow) {
        "processCheckoutAutocapture" -> processCheckoutAutocapture(txnId)
        "processCheckoutCard" -> processCheckoutCard(txnId)
        "processRefund" -> processRefund(txnId)
        "processGetPayment" -> processGetPayment(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: processCheckoutAutocapture, processCheckoutCard, processRefund, processGetPayment")
    }
}
