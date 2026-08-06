// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py grabpay
//
// Grabpay — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="grabpay processCheckoutCard"

package examples.grabpay

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.EventClient
import payments.Currency
import payments.HttpMethod
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.GrabpayConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("create_order", "parse_event")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setGrabpay(GrabpayConfig.newBuilder()
                .setPartnerId(SecretString.newBuilder().setValue("YOUR_PARTNER_ID").build())
                .setPartnerSecret(SecretString.newBuilder().setValue("YOUR_PARTNER_SECRET").build())
                .setClientId(SecretString.newBuilder().setValue("YOUR_CLIENT_ID").build())
                .setClientSecret(SecretString.newBuilder().setValue("YOUR_CLIENT_SECRET").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()


// Flow: PaymentService.CreateOrder
fun createOrder(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
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

// Flow: EventService.HandleEvent
fun handleEvent(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = EventClient(config)
    val request = EventServiceHandleRequest.newBuilder().apply {
        merchantEventId = "probe_event_001"  // Caller-supplied correlation key, echoed in the response. Not used by UCS for processing.
        requestDetailsBuilder.apply {
            method = HttpMethod.HTTP_METHOD_POST  // HTTP method of the request (e.g., GET, POST).
            uri = "https://example.com/webhook"  // URI of the request.
            putAllHeaders(mapOf())  // Headers of the HTTP request.
            body = com.google.protobuf.ByteString.copyFromUtf8("{\"txType\":\"payment\",\"txStatus\":\"success\",\"partnerID\":\"partner_123\",\"partnerTxID\":\"txn_123\",\"txID\":\"grab_txn_123\",\"amount\":100,\"currency\":\"SGD\",\"payload\":{\"newStatus\":\"success\",\"paymentMethod\":\"GRABPAY\"}}")  // Body of the HTTP request.
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
            body = com.google.protobuf.ByteString.copyFromUtf8("{\"txType\":\"payment\",\"txStatus\":\"success\",\"partnerID\":\"partner_123\",\"partnerTxID\":\"txn_123\",\"txID\":\"grab_txn_123\",\"amount\":100,\"currency\":\"SGD\",\"payload\":{\"newStatus\":\"success\",\"paymentMethod\":\"GRABPAY\"}}")  // Body of the HTTP request.
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
    val flow = args.firstOrNull() ?: "createOrder"
    when (flow) {
        "createOrder" -> createOrder(txnId)
        "handleEvent" -> handleEvent(txnId)
        "parseEvent" -> parseEvent(txnId)
        "verifyRedirect" -> verifyRedirect(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: createOrder, handleEvent, parseEvent, verifyRedirect")
    }
}
