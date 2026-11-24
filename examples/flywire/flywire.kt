// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py flywire
//
// Flywire — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="flywire processCheckoutCard"

package examples.flywire

import types.Payment.*
import types.Events.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.EventClient
import payments.RefundClient
import payments.Currency
import payments.HttpMethod
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.FlywireConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("get", "parse_event", "refund", "refund_get")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setFlywire(FlywireConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setSharedSecret(SecretString.newBuilder().setValue("YOUR_SHARED_SECRET").build())
                .setRecipientId("YOUR_RECIPIENT_ID")
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

private fun buildRefundRequest(connectorTransactionIdStr: String): PaymentServiceRefundRequest {
    return PaymentServiceRefundRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        paymentAmount = 1000L  // Amount Information.
        refundAmountBuilder.apply {
            minorAmount = 1000L
            currency = Currency.USD
        }
        reason = "customer_request"  // Reason for the refund.
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
            uri = "https://example.com/webhook"  // URI of the request.
            putAllHeaders(mapOf())  // Headers of the HTTP request.
            body = com.google.protobuf.ByteString.copyFromUtf8("{\"event_type\":\"guaranteed\",\"event_date\":\"2026-01-01T00:00:00Z\",\"event_resource\":\"payments\",\"data\":{\"payment_id\":\"probe_pay_001\",\"status\":\"guaranteed\",\"external_reference\":\"probe_ref_001\"}}")  // Body of the HTTP request.
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
            body = com.google.protobuf.ByteString.copyFromUtf8("{\"event_type\":\"guaranteed\",\"event_date\":\"2026-01-01T00:00:00Z\",\"event_resource\":\"payments\",\"data\":{\"payment_id\":\"probe_pay_001\",\"status\":\"guaranteed\",\"external_reference\":\"probe_ref_001\"}}")  // Body of the HTTP request.
        }
    }.build()
    val response = client.parse_event(request)
    println("Webhook parsed: type=${response.eventType.name}")
}

// Flow: PaymentService.Refund
fun refund(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildRefundRequest("probe_connector_txn_001")
    val response = client.refund(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Refund failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}

// Flow: RefundService.Get
fun refundGet(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = RefundClient(config)
    val request = RefundServiceGetRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"  // Identification.
        connectorTransactionId = "probe_connector_txn_001"
        refundId = "probe_refund_id_001"  // Deprecated.
    }.build()
    val response = client.refund_get(request)
    println("Status: ${response.status.name}")
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
    val flow = args.firstOrNull() ?: "get"
    when (flow) {
        "get" -> get(txnId)
        "handleEvent" -> handleEvent(txnId)
        "parseEvent" -> parseEvent(txnId)
        "refund" -> refund(txnId)
        "refundGet" -> refundGet(txnId)
        "verifyRedirect" -> verifyRedirect(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: get, handleEvent, parseEvent, refund, refundGet, verifyRedirect")
    }
}
