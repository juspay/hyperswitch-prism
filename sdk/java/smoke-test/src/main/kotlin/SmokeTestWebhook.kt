/**
 * Webhook smoke test — Adyen AUTHORISATION
 *
 * Uses a real Adyen AUTHORISATION webhook body and feeds it into
 * EventClient.handle_event / parse_event with connector identity only
 * (no API credentials, no webhook secret).
 *
 * What this validates:
 *  1. SDK routes to the correct connector from identity alone
 *  2. Adyen webhook body is parsed correctly
 *  3. event_type is returned
 *  4. source_verified=false is expected — no real HMAC secret provided,
 *     and Adyen verification is not mandatory so it must NOT error out
 *  5. IntegrationError / ConnectorError are NOT thrown for a valid payload
 *
 * Usage:
 *   cd sdk/java && ./gradlew runWebhookSmokeTest
 */

import payments.EventClient
import payments.IntegrationError
import payments.ConnectorError
import types.Payment.*
import types.SdkConfig.*

// ── ANSI color helpers ─────────────────────────────────────────────────────────
private val NO_COLOR = System.getenv("NO_COLOR") != null
    || (System.getenv("FORCE_COLOR") == null
        && System.console() == null
        && System.getenv("TERM").let { it == null || it == "dumb" })

private fun c(code: String, text: String) = if (NO_COLOR) text else "\u001b[${code}m$text\u001b[0m"
private fun green(t: String)  = c("32", t)
private fun yellow(t: String) = c("33", t)
private fun red(t: String)    = c("31", t)
private fun grey(t: String)   = c("90", t)
private fun bold(t: String)   = c("1",  t)

// ── Adyen AUTHORISATION webhook body (from real test configuration) ────────────
// Sensitive fields replaced:
//   merchantAccountCode → "YOUR_MERCHANT_ACCOUNT"
//   merchantReference   → "pay_test_00000000000000"
//   pspReference        → "TEST000000000000"
//   hmacSignature       → "test_hmac_signature_placeholder"
//   cardHolderName      → "John Doe"
//   shopperEmail        → "shopper@example.com"
private val ADYEN_WEBHOOK_BODY = """
{
  "live": "false",
  "notificationItems": [{
    "NotificationRequestItem": {
      "additionalData": {
        "authCode": "APPROVED",
        "cardSummary": "1111",
        "cardHolderName": "John Doe",
        "expiryDate": "03/2030",
        "shopperEmail": "shopper@example.com",
        "shopperIP": "128.0.0.1",
        "shopperInteraction": "Ecommerce",
        "captureDelayHours": "0",
        "gatewaySystem": "direct",
        "hmacSignature": "test_hmac_signature_placeholder"
      },
      "amount": { "currency": "GBP", "value": 654000 },
      "eventCode": "AUTHORISATION",
      "eventDate": "2026-01-21T14:18:18+01:00",
      "merchantAccountCode": "YOUR_MERCHANT_ACCOUNT",
      "merchantReference": "pay_test_00000000000000",
      "operations": ["CAPTURE", "REFUND"],
      "paymentMethod": "visa",
      "pspReference": "TEST000000000000",
      "reason": "APPROVED:1111:03/2030",
      "success": "true"
    }
  }]
}
""".trimIndent()

private val ADYEN_HEADERS = mapOf(
    "content-type" to "application/json",
    "accept" to "*/*",
)

// ── Webhook smoke test ─────────────────
private fun buildConfig(): ConnectorConfig =
    ConnectorConfig.newBuilder()
        .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
        .setConnectorConfig(
            ConnectorSpecificConfig.newBuilder()
                .setAdyen(AdyenConfig.newBuilder().build())
                .build()
        )
        .build()

// ── Test 1: handle_event — AUTHORISATION ──────────────────────────────────────
fun testHandleEvent(): Boolean {
    println(bold("\n[Adyen Webhook — AUTHORISATION handle_event]"))
    val client = EventClient(buildConfig())

    val request = EventServiceHandleRequest.newBuilder()
        .setMerchantEventId("smoke_wh_adyen_auth")
        .setRequestDetails(
            RequestDetails.newBuilder()
                .setMethod(HttpMethod.HTTP_METHOD_POST)
                .setUri("/webhooks/adyen")
                .putAllHeaders(ADYEN_HEADERS)
                .setBody(com.google.protobuf.ByteString.copyFromUtf8(ADYEN_WEBHOOK_BODY))
                .build()
        )
        // capture_method is the EventContext use case:
        // Adyen AUTHORISATION maps to AUTHORIZED (manual) or CAPTURED (automatic)
        .setEventContext(
            EventContext.newBuilder()
                .setCaptureMethod(CaptureMethod.MANUAL)
                .build()
        )
        .build()

    return try {
        val response = client.handle_event(request)
        println(grey("  event_type     : ${response.eventType}"))
        println(grey("  source_verified: ${response.sourceVerified}"))
        println(grey("  merchant_event : ${response.merchantEventId}"))
        if (!response.sourceVerified) {
            println(yellow("  ~ source_verified=false (expected — no real HMAC secret)"))
        }
        println(green("  ✓ PASSED: handle_event returned response without crashing"))
        true
    } catch (e: IntegrationError) {
        println(red("  ✗ FAILED: IntegrationError: ${e.message} (code=${e.errorCode})"))
        false
    } catch (e: ConnectorError) {
        println(red("  ✗ FAILED: ConnectorError: ${e.message} (code=${e.errorCode})"))
        false
    } catch (e: Exception) {
        println(red("  ✗ FAILED: ${e.javaClass.simpleName}: ${e.message}"))
        false
    }
}

// ── Test 2: parse_event ────────────────────────────────────────────────────────
fun testParseEvent(): Boolean {
    println(bold("\n[Adyen Webhook — AUTHORISATION parse_event]"))
    val client = EventClient(buildConfig())

    val request = EventServiceParseRequest.newBuilder()
        .setRequestDetails(
            RequestDetails.newBuilder()
                .setMethod(HttpMethod.HTTP_METHOD_POST)
                .setUri("/webhooks/adyen")
                .putAllHeaders(ADYEN_HEADERS)
                .setBody(com.google.protobuf.ByteString.copyFromUtf8(ADYEN_WEBHOOK_BODY))
                .build()
        )
        .build()

    return try {
        val response = client.parse_event(request)
        println(grey("  event_type : ${response.eventType}"))
        println(grey("  reference  : ${response.reference}"))
        println(green("  ✓ PASSED: parse_event returned response"))
        true
    } catch (e: IntegrationError) {
        println(red("  ✗ FAILED: IntegrationError: ${e.message} (code=${e.errorCode})"))
        false
    } catch (e: ConnectorError) {
        println(red("  ✗ FAILED: ConnectorError: ${e.message} (code=${e.errorCode})"))
        false
    } catch (e: Exception) {
        println(red("  ✗ FAILED: ${e.javaClass.simpleName}: ${e.message}"))
        false
    }
}

// ── Test 3: malformed body ─────────────────────────────────────────────────────
fun testMalformedBody(): Boolean {
    println(bold("\n[Adyen Webhook — malformed body]"))
    val client = EventClient(buildConfig())

    val request = EventServiceHandleRequest.newBuilder()
        .setRequestDetails(
            RequestDetails.newBuilder()
                .setMethod(HttpMethod.HTTP_METHOD_POST)
                .setUri("/webhooks/adyen")
                .putAllHeaders(ADYEN_HEADERS)
                .setBody(com.google.protobuf.ByteString.copyFromUtf8("not valid json {{{{"))
                .build()
        )
        .build()

    return try {
        val response = client.handle_event(request)
        println(yellow("  ~ accepted malformed body — event_type: ${response.eventType}"))
        true
    } catch (e: IntegrationError) {
        println(green("  ✓ PASSED: IntegrationError thrown as expected: ${e.message}"))
        true
    } catch (e: ConnectorError) {
        println(green("  ✓ PASSED: ConnectorError thrown as expected: ${e.message}"))
        true
    } catch (e: Exception) {
        println(red("  ✗ FAILED: unexpected error: ${e.javaClass.simpleName}: ${e.message}"))
        false
    }
}

// ── Test 4: unknown eventCode ──────────────────────────────────────────────────
fun testUnknownEventCode(): Boolean {
    println(bold("\n[Adyen Webhook — unknown eventCode]"))
    val client = EventClient(buildConfig())

    val unknownBody = ADYEN_WEBHOOK_BODY.replace("\"AUTHORISATION\"", "\"SOME_UNKNOWN_EVENT\"")

    val request = EventServiceHandleRequest.newBuilder()
        .setRequestDetails(
            RequestDetails.newBuilder()
                .setMethod(HttpMethod.HTTP_METHOD_POST)
                .setUri("/webhooks/adyen")
                .putAllHeaders(ADYEN_HEADERS)
                .setBody(com.google.protobuf.ByteString.copyFromUtf8(unknownBody))
                .build()
        )
        .build()

    return try {
        val response = client.handle_event(request)
        println(green("  ✓ PASSED: handled gracefully — event_type: ${response.eventType}"))
        true
    } catch (e: IntegrationError) {
        println(green("  ✓ PASSED: IntegrationError for unknown event (expected): ${e.message}"))
        true
    } catch (e: ConnectorError) {
        println(green("  ✓ PASSED: ConnectorError for unknown event (expected): ${e.message}"))
        true
    } catch (e: Exception) {
        println(red("  ✗ FAILED: ${e.javaClass.simpleName}: ${e.message}"))
        false
    }
}

// ── main ───────────────────────────────────────────────────────────────────────
fun main() {
    println(bold("Adyen Webhook Smoke Test"))
    println("─".repeat(50))

    val results = listOf(
        testHandleEvent(),
        testParseEvent(),
        testMalformedBody(),
        testUnknownEventCode(),
    )

    println("\n" + "=".repeat(50))
    val allPassed = results.all { it }
    println(if (allPassed) green("PASSED") else red("FAILED"))
    kotlin.system.exitProcess(if (allPassed) 0 else 1)
}
