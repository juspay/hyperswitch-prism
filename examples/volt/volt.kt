// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py volt
//
// Volt — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="volt processCheckoutCard"

package examples.volt

import payments.MerchantAuthenticationClient
import payments.FraudClient
import payments.PaymentClient
import payments.MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
import payments.FraudServiceGetRequest
import payments.PaymentServiceRefundRequest
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment


private fun buildGetRequest(connectorTransactionIdStr: String): FraudServiceGetRequest {
    return FraudServiceGetRequest.newBuilder().apply {
        merchantTransactionId = "probe_merchant_txn_001"
        connectorTransactionId = connectorTransactionIdStr
        minorAmount = 1000L
        currency = "USD"
        token = "probe_access_token"
        expiresInSeconds = 3600L
        tokenType = "Bearer"
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
        stateBuilder.apply {  // State data for access token storage and.
            accessTokenBuilder.apply {  // Access token obtained from connector.
                tokenBuilder.value = "probe_access_token"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch).
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    }.build()
}

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your connector config here
    .build()


// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
fun createServerAuthenticationToken(txnId: String) {
    val client = MerchantAuthenticationClient(_defaultConfig)
    val request = MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest.newBuilder().apply {

    }.build()
    val response = client.create_server_authentication_token(request)
    println("Status: ${response.status.name}")
}

// Flow: FraudService.Get
fun get(txnId: String) {
    val client = FraudClient(_defaultConfig)
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


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "createServerAuthenticationToken"
    when (flow) {
        "createServerAuthenticationToken" -> createServerAuthenticationToken(txnId)
        "get" -> get(txnId)
        "refund" -> refund(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: createServerAuthenticationToken, get, refund")
    }
}
