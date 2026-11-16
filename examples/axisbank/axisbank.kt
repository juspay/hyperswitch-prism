// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py axisbank
//
// Axisbank — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="axisbank processCheckoutCard"

package examples.axisbank

import types.Payment.*
import types.Events.*
import types.PaymentMethods.*
import payments.EventClient
import payments.HttpMethod
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.AxisbankConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("parse_event")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setAxisbank(AxisbankConfig.newBuilder()
                .setMerchantKid(SecretString.newBuilder().setValue("YOUR_MERCHANT_KID").build())
                .setJuspayKid(SecretString.newBuilder().setValue("YOUR_JUSPAY_KID").build())
                .setMerchantPrivateKey(SecretString.newBuilder().setValue("YOUR_MERCHANT_PRIVATE_KEY").build())
                .setJuspayPublicKey(SecretString.newBuilder().setValue("YOUR_JUSPAY_PUBLIC_KEY").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()


// Flow: EventService.ParseEvent
fun parseEvent(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = EventClient(config)
    val request = EventServiceParseRequest.newBuilder().apply {
        requestDetailsBuilder.apply {
            method = HttpMethod.HTTP_METHOD_POST  // HTTP method of the request (e.g., GET, POST).
            uri = "https://example.com/webhook"  // URI of the request.
            putAllHeaders(mapOf())  // Headers of the HTTP request.
            body = com.google.protobuf.ByteString.copyFromUtf8("{}")  // Body of the HTTP request.
        }
    }.build()
    val response = client.parse_event(request)
    println("Webhook parsed: type=${response.eventType.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "parseEvent"
    when (flow) {
        "parseEvent" -> parseEvent(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: parseEvent")
    }
}
