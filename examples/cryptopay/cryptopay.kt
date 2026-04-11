// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py cryptopay
//
// Cryptopay — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="cryptopay processCheckoutCard"

package examples.cryptopay

import payments.FraudClient
import payments.EventClient
import payments.FraudServiceGetRequest
import payments.EventServiceHandleRequest
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment


private fun buildGetRequest(connectorTransactionIdStr: String): FraudServiceGetRequest {
    return FraudServiceGetRequest.newBuilder().apply {
        merchantTransactionId = "probe_merchant_txn_001"
        connectorTransactionId = connectorTransactionIdStr
        minorAmount = 1000L
        currency = "USD"
    }.build()
}

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your connector config here
    .build()


// Flow: FraudService.Get
fun get(txnId: String) {
    val client = FraudClient(_defaultConfig)
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


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "get"
    when (flow) {
        "get" -> get(txnId)
        "handleEvent" -> handleEvent(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: get, handleEvent")
    }
}
