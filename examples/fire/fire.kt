// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py fire
//
// Fire — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="fire processCheckoutCard"

package examples.fire

import types.Payment.*
import types.Events.*
import types.PaymentMethods.*
import payments.MerchantAuthenticationClient
import payments.EventClient
import payments.PaymentClient
import payments.HttpMethod
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.FireConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("create_server_authentication_token", "parse_event")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setFire(FireConfig.newBuilder()
                .setClientId(SecretString.newBuilder().setValue("YOUR_CLIENT_ID").build())
                .setClientKey(SecretString.newBuilder().setValue("YOUR_CLIENT_KEY").build())
                .setRefreshToken(SecretString.newBuilder().setValue("YOUR_REFRESH_TOKEN").build())
                .setIcanTo(SecretString.newBuilder().setValue("YOUR_ICAN_TO").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setWebhookSecret(SecretString.newBuilder().setValue("YOUR_WEBHOOK_SECRET").build())
                .build())
            .build()
    )
    .build()


// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
fun createServerAuthenticationToken(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = MerchantAuthenticationClient(config)
    val request = MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest.newBuilder().apply {

    }.build()
    val response = client.create_server_authentication_token(request)
    println("StatusCode: ${response.statusCode}")
}

// Flow: EventService.HandleEvent
fun handleEvent(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = EventClient(config)
    val request = EventServiceHandleRequest.newBuilder().apply {
        merchantEventId = "probe_event_001"
        requestDetailsBuilder.apply {
            method = HttpMethod.HTTP_METHOD_POST  // HTTP method of the request (e.g., GET, POST).
            uri = "https://example.com/webhook"  // URI of the request.
            putAllHeaders(mapOf())  // Headers of the HTTP request.
            body = com.google.protobuf.ByteString.copyFromUtf8("eyJraWQiOiI2NjAzZWQzYi1iYzMwLTQ3ZmItYjVlMy1iMjFjOWFiYmNlYzkiLCJhbGciOiJIUzI1NiJ9.eyJ0eXBlIjoiT1RIRVIiLCJzdGF0dXMiOiJBVVRIT1JJU0VEIiwidHJhbnNhY3Rpb25UeXBlIjoiUEFZTUVOVCIsInBheW1lbnRVdWlkIjoiM2YyZDBhMjAtZTg0Yi00MWMxLTlkM2QtYTFiMmMzZDRlNWY2In0.c2lnbmF0dXJl")  // Body of the HTTP request.
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
            body = com.google.protobuf.ByteString.copyFromUtf8("eyJraWQiOiI2NjAzZWQzYi1iYzMwLTQ3ZmItYjVlMy1iMjFjOWFiYmNlYzkiLCJhbGciOiJIUzI1NiJ9.eyJ0eXBlIjoiT1RIRVIiLCJzdGF0dXMiOiJBVVRIT1JJU0VEIiwidHJhbnNhY3Rpb25UeXBlIjoiUEFZTUVOVCIsInBheW1lbnRVdWlkIjoiM2YyZDBhMjAtZTg0Yi00MWMxLTlkM2QtYTFiMmMzZDRlNWY2In0.c2lnbmF0dXJl")  // Body of the HTTP request.
        }
    }.build()
    val response = client.parse_event(request)
    println("Webhook parsed: type=${response.eventType.name}")
}

// Flow: PaymentService.VerifyRedirectResponse
fun verifyRedirect(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceVerifyRedirectResponseRequest.newBuilder().apply {

    }.build()
    val response = client.verify_redirect_response(request)
    println("Source verified: ${response.sourceVerified}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "createServerAuthenticationToken"
    when (flow) {
        "createServerAuthenticationToken" -> createServerAuthenticationToken(txnId)
        "handleEvent" -> handleEvent(txnId)
        "parseEvent" -> parseEvent(txnId)
        "verifyRedirect" -> verifyRedirect(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: createServerAuthenticationToken, handleEvent, parseEvent, verifyRedirect")
    }
}
