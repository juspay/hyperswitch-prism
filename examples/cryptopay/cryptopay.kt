// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py cryptopay
//
// Cryptopay — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="cryptopay processCheckoutCard"

package examples.cryptopay

import types.Payment.*
import types.Events.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.EventClient
import payments.Currency
import payments.HttpMethod
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.CryptopayConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("get", "parse_event")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setCryptopay(CryptopayConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setApiSecret(SecretString.newBuilder().setValue("YOUR_API_SECRET").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()



private fun buildGetRequest(connectorTransactionIdStr: String): PaymentServiceGetRequest {
    return PaymentServiceGetRequest.newBuilder().apply {
        merchantTransactionId = "probe_merchant_txn_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L
            currency = Currency.USD
        }
    }.build()
}

// Flow: PaymentService.Get
fun get(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildGetRequest("probe_connector_txn_001")
    val response = client.get(request)
    println("Status: ${response.status.name}")
}

// Flow: EventService.HandleEvent
fun handleEvent(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = EventClient(config)
    val request = EventServiceHandleRequest.newBuilder().apply {
        merchantEventId = "probe_event_001"
        requestDetailsBuilder.apply {
            method = HttpMethod.HTTP_METHOD_POST  // HTTP method of the request (e.g., GET, POST).
            putAllLegacyHeaders(mapOf())  // Headers of the HTTP request.
            bodyBuilder.value = com.google.protobuf.ByteString.copyFromUtf8("{\"type\":\"Invoice\",\"event\":\"status_changed\",\"data\":{\"id\":\"probe_invoice_001\",\"status\":\"completed\",\"price_amount\":\"10.00\",\"price_currency\":\"USD\",\"name\":\"probe_charge\"}}")  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            uriBuilder.value = "https://example.com/webhook"
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
            putAllLegacyHeaders(mapOf())  // Headers of the HTTP request.
            bodyBuilder.value = com.google.protobuf.ByteString.copyFromUtf8("{\"type\":\"Invoice\",\"event\":\"status_changed\",\"data\":{\"id\":\"probe_invoice_001\",\"status\":\"completed\",\"price_amount\":\"10.00\",\"price_currency\":\"USD\",\"name\":\"probe_charge\"}}")  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            uriBuilder.value = "https://example.com/webhook"
        }
    }.build()
    val response = client.parse_event(request)
    println("Webhook parsed: type=${response.eventType.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "get"
    when (flow) {
        "get" -> get(txnId)
        "handleEvent" -> handleEvent(txnId)
        "parseEvent" -> parseEvent(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: get, handleEvent, parseEvent")
    }
}
