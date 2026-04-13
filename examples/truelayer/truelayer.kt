// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py truelayer
//
// Truelayer — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="truelayer processCheckoutCard"

package examples.truelayer

import payments.MerchantAuthenticationClient
import payments.PaymentClient
import payments.EventClient
import payments.RefundClient
import payments.MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
import payments.PaymentServiceGetRequest
import payments.EventServiceHandleRequest
import payments.RefundServiceGetRequest
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment


private fun buildGetRequest(connectorTransactionIdStr: String): PaymentServiceGetRequest {
    return PaymentServiceGetRequest.newBuilder().apply {
        merchantTransactionId = "probe_merchant_txn_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        stateBuilder.apply {  // State Information.
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

// Flow: PaymentService.Get
fun get(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildGetRequest("probe_connector_txn_001")
    val response = client.get(request)
    println("Status: ${response.status.name}")
}

// Flow: EventService.HandleEvent
fun handleEvent(txnId: String) {
    val client = EventClient(_defaultConfig)
    val request = EventServiceHandleRequest.newBuilder().apply {

    }.build()
    val response = client.handle_event(request)
    println("Status: ${response.status.name}")
}

// Flow: RefundService.Get
fun refundGet(txnId: String) {
    val client = RefundClient(_defaultConfig)
    val request = RefundServiceGetRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"  // Identification.
        connectorTransactionId = "probe_connector_txn_001"
        refundId = "probe_refund_id_001"
        stateBuilder.apply {  // State Information.
            accessTokenBuilder.apply {  // Access token obtained from connector.
                tokenBuilder.value = "probe_access_token"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch).
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    }.build()
    val response = client.refund_get(request)
    println("Status: ${response.status.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "createServerAuthenticationToken"
    when (flow) {
        "createServerAuthenticationToken" -> createServerAuthenticationToken(txnId)
        "get" -> get(txnId)
        "handleEvent" -> handleEvent(txnId)
        "refundGet" -> refundGet(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: createServerAuthenticationToken, get, handleEvent, refundGet")
    }
}
