// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py cashtocode
//
// Cashtocode — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="cashtocode processCheckoutCard"

package examples.cashtocode

import types.Payment.*
import types.PaymentMethods.*
import payments.EventClient
import payments.HttpMethod
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.CashtocodeConfig

val SUPPORTED_FLOWS = listOf<String>("parse_event")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setCashtocode(CashtocodeConfig.newBuilder()
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()


// Flow: EventService.HandleEvent
fun handleEvent(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = EventClient(config)
    val request = EventServiceHandleRequest.newBuilder().apply {
        merchantEventId = "probe_event_001"  // Caller-supplied correlation key, echoed in the response. Not used by UCS for processing.
        requestDetailsBuilder.apply {
            method = HttpMethod.HTTP_METHOD_POST  // HTTP method of the request (e.g., GET, POST).
            uri = "https://example.com/webhook"  // URI of the request.
            putAllHeaders(mapOf())  // Headers of the HTTP request.
            body = com.google.protobuf.ByteString.copyFromUtf8("{\"amount\":10.0,\"currency\":\"EUR\",\"foreignTransactionId\":\"probe_foreign_001\",\"type\":\"payment\",\"transactionId\":\"probe_txn_001\"}")  // Body of the HTTP request.
        }
    }.build()
    val response = client.handle_event(request)
    println("Webhook: type=${response.eventType.name} verified=${response.sourceVerified}")
}

// Flow: EventService.ParseEvent
fun parseEvent(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = EventClient(config)
    val request = EventServiceParseRequest.newBuilder().apply {
        requestDetailsBuilder.apply {
            method = HttpMethod.HTTP_METHOD_POST  // HTTP method of the request (e.g., GET, POST).
            uri = "https://example.com/webhook"  // URI of the request.
            putAllHeaders(mapOf())  // Headers of the HTTP request.
            body = com.google.protobuf.ByteString.copyFromUtf8("{\"amount\":10.0,\"currency\":\"EUR\",\"foreignTransactionId\":\"probe_foreign_001\",\"type\":\"payment\",\"transactionId\":\"probe_txn_001\"}")  // Body of the HTTP request.
        }
    }.build()
    val response = client.parse_event(request)
    println("Webhook parsed: type=${response.eventType.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "handleEvent"
    when (flow) {
        "handleEvent" -> handleEvent(txnId)
        "parseEvent" -> parseEvent(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: handleEvent, parseEvent")
    }
}
