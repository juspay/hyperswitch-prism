// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py forte
//
// Forte — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="forte processCheckoutCard"

package examples.forte

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment


val SUPPORTED_FLOWS = listOf<String>("authorize", "get", "proxy_authorize", "refund_get", "void")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Forte credentials here
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
        firstName = "John"
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
    }.build())

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    return mapOf("status" to authorizeResponse.status.name, "transactionId" to authorizeResponse.connectorTransactionId, "error" to authorizeResponse.error)
}

// Scenario: Void Payment
// Cancel an authorized but not-yet-captured payment.
fun processVoidPayment(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
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
        firstName = "John"
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
    }.build())

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    // Step 2: Void — release reserved funds (cancel authorization)
    val voidResponse = paymentClient.void(.newBuilder().apply {
        merchantVoidId = "probe_void_001"
        connectorTransactionId = authorizeResponse.connectorTransactionId  // from Authorize
    }.build())

    return mapOf("status" to voidResponse.status.name, "transactionId" to authorizeResponse.connectorTransactionId, "error" to voidResponse.error)
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
        firstName = "John"
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
        "processVoidPayment" -> processVoidPayment(txnId)
        "processGetPayment" -> processGetPayment(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: processCheckoutAutocapture, processVoidPayment, processGetPayment")
    }
}
