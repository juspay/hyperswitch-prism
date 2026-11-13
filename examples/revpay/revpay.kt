// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py revpay
//
// Revpay — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="revpay processCheckoutCard"

package examples.revpay

import types.Payment.*
import types.Events.*
import types.PaymentMethods.*
import payments.EventClient
import payments.HttpMethod
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.RevpayConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("parse_event")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setRevpay(RevpayConfig.newBuilder()
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setMerchantKey(SecretString.newBuilder().setValue("YOUR_MERCHANT_KEY").build())
                .setKeyIndex(SecretString.newBuilder().setValue("YOUR_KEY_INDEX").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()


// Flow: EventService.HandleEvent
fun handleEvent(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = EventClient(config)
    val request = EventServiceHandleRequest.newBuilder().apply {
        merchantEventId = "probe_event_001"
        requestDetailsBuilder.apply {
            method = HttpMethod.HTTP_METHOD_POST  // HTTP method of the request (e.g., GET, POST).
            uri = "https://example.com/webhook"  // URI of the request.
            putAllHeaders(mapOf())  // Headers of the HTTP request.
            body = com.google.protobuf.ByteString.copyFromUtf8("Revpay_Merchant_ID=MER00000000001&Payment_ID=2&Bank_Code=MB2U&Transaction_ID=RT000000007222&Reference_Number=RF201774111&Bank_Reference_Number=BR000001&Amount=52.40&Currency=MYR&Transaction_Description=Order+RF201774111&Fraud_Score=0&Response_Code=00&Error_Description=&Key_Index=1&Signature=fc8d556ae2e4fa12836e1192a4f0a793a9672af1d2348da502ef299a6b5169f4402fb6bb282a9e097fe1253b4bfd4f2fa365b7d593b564c585fc79b6c302ecad&Request_Datetime=2020-01-01+12%3A00%3A00&Response_Datetime=2020-01-01+12%3A00%3A05")  // Body of the HTTP request.
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
            body = com.google.protobuf.ByteString.copyFromUtf8("Revpay_Merchant_ID=MER00000000001&Payment_ID=2&Bank_Code=MB2U&Transaction_ID=RT000000007222&Reference_Number=RF201774111&Bank_Reference_Number=BR000001&Amount=52.40&Currency=MYR&Transaction_Description=Order+RF201774111&Fraud_Score=0&Response_Code=00&Error_Description=&Key_Index=1&Signature=fc8d556ae2e4fa12836e1192a4f0a793a9672af1d2348da502ef299a6b5169f4402fb6bb282a9e097fe1253b4bfd4f2fa365b7d593b564c585fc79b6c302ecad&Request_Datetime=2020-01-01+12%3A00%3A00&Response_Datetime=2020-01-01+12%3A00%3A05")  // Body of the HTTP request.
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
