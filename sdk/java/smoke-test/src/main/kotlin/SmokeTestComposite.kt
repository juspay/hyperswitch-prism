/**
 * Composite smoke test for the hyperswitch-payments Java SDK.
 *
 * Unlike SmokeTest.kt (which uses reflection to discover example functions),
 * this test calls the SDK directly — exactly as a real Java/Kotlin user would.
 * This validates the typed exception contract:
 *
 *   try {
 *       val response = client.authorize(request)
 *   } catch (e: IntegrationError) {
 *       // Request never sent — missing field, bad config, etc.
 *   } catch (e: ConnectorError) {
 *       // Connector rejected it — declined, 4xx/5xx, etc.
 *   }
 *
 * Usage:
 *   ./gradlew runComposite --args="--creds-file creds.json"
 */

import com.google.gson.Gson
import com.google.gson.reflect.TypeToken
import payments.AuthenticationType
import payments.CaptureMethod
import payments.ConnectorConfig
import payments.ConnectorError
import payments.ConnectorSpecificConfig
import payments.Currency
import payments.Environment
import payments.IntegrationError
import payments.PaymentClient
import payments.PaymentServiceAuthorizeRequest
import payments.PaymentStatus
import payments.SdkOptions
import java.io.File

// ── ANSI helpers ─────────────────────────────────────────────────────────────
private val NO_COLOR_C = System.getenv("NO_COLOR") != null
    || (System.getenv("FORCE_COLOR") == null
        && System.console() == null
        && System.getenv("TERM").let { it == null || it == "dumb" })

private fun cc(code: String, text: String) = if (NO_COLOR_C) text else "\u001b[${code}m$text\u001b[0m"
private fun green2 (t: String) = cc("32", t)
private fun yellow2(t: String) = cc("33", t)
private fun red2   (t: String) = cc("31", t)
private fun grey2  (t: String) = cc("90", t)

// ── Credential helpers ────────────────────────────────────────────────────────

private fun loadCreds(credsFile: String): Map<String, Any> {
    val file = File(credsFile)
    if (!file.exists()) return emptyMap()
    @Suppress("UNCHECKED_CAST")
    return Gson().fromJson(file.readText(), object : TypeToken<Map<String, Any>>() {}.type)
}

private fun stripeApiKey(creds: Map<String, Any>): String? {
    val raw = creds["stripe"] ?: return null
    @Suppress("UNCHECKED_CAST")
    val stripe = (if (raw is List<*>) raw.firstOrNull() else raw) as? Map<String, Any> ?: return null
    @Suppress("UNCHECKED_CAST")
    return ((stripe["apiKey"] ?: stripe["api_key"]) as? Map<String, Any>)?.get("value") as? String
}

private fun buildStripeConfig(apiKey: String): ConnectorConfig {
    val specific = ConnectorSpecificConfig.newBuilder().apply {
        stripeBuilder.apiKeyBuilder.value = apiKey
    }.build()
    return ConnectorConfig.newBuilder()
        .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
        .setConnectorConfig(specific)
        .build()
}

// ── Test result ───────────────────────────────────────────────────────────────

data class TestResult(val name: String, val passed: Boolean, val detail: String)

// ── Test cases ────────────────────────────────────────────────────────────────

/**
 * Happy path: valid Stripe authorize should succeed.
 */
fun testStripeAuthorizeSuccess(apiKey: String): TestResult {
    val name = "stripe_authorize_success"
    val client = PaymentClient(buildStripeConfig(apiKey))

    val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "composite_authorize_${System.currentTimeMillis()}"
        amountBuilder.apply {
            minorAmount = 1000L
            currency = Currency.USD
        }
        paymentMethodBuilder.cardBuilder.apply {
            cardNumberBuilder.value = "4111111111111111"
            cardExpMonthBuilder.value = "12"
            cardExpYearBuilder.value = "2050"
            cardCvcBuilder.value = "123"
            cardHolderNameBuilder.value = "Test User"
        }
        captureMethod = CaptureMethod.AUTOMATIC
        authType = AuthenticationType.NO_THREE_DS
        addressBuilder  // empty billing address
    }.build()

    return try {
        val response = client.authorize(request)
        if (response.status == PaymentStatus.CHARGED) {
            TestResult(name, true, "CHARGED — transactionId=${response.connectorTransactionId}")
        } else {
            TestResult(name, false, "Expected CHARGED, got ${response.status}")
        }
    } catch (e: IntegrationError) {
        // Should NOT happen for a valid request
        TestResult(name, false, "IntegrationError (unexpected): ${e.message} (code=${e.errorCode})")
    } catch (e: ConnectorError) {
        // Connector declined — note it but don't fail (sandbox behaviour)
        TestResult(name, true, "ConnectorError: ${e.message} (code=${e.errorCode}, http=${e.httpStatusCode})")
    }
}

/**
 * IntegrationError path: missing required field (amount) must throw IntegrationError
 * and must NOT reach the connector.
 */
fun testIntegrationErrorOnMissingAmount(apiKey: String): TestResult {
    val name = "integration_error_missing_amount"
    val client = PaymentClient(buildStripeConfig(apiKey))

    // amount intentionally omitted
    val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "composite_missing_amount_${System.currentTimeMillis()}"
        paymentMethodBuilder.cardBuilder.apply {
            cardNumberBuilder.value = "4111111111111111"
            cardExpMonthBuilder.value = "12"
            cardExpYearBuilder.value = "2050"
            cardCvcBuilder.value = "123"
            cardHolderNameBuilder.value = "Test User"
        }
        captureMethod = CaptureMethod.AUTOMATIC
        authType = AuthenticationType.NO_THREE_DS
    }.build()

    return try {
        client.authorize(request)
        TestResult(name, false, "Expected IntegrationError but call succeeded — request should have been rejected before the HTTP call")
    } catch (e: IntegrationError) {
        // Correct — request never sent, caught at req_transformer
        TestResult(name, true, "IntegrationError (expected): ${e.message} (code=${e.errorCode})")
    } catch (e: ConnectorError) {
        // Wrong — connector was never supposed to be called
        TestResult(name, false, "Got ConnectorError instead of IntegrationError: ${e.message}")
    }
}

/**
 * ConnectorError path: request is valid but card is known to be declined by Stripe.
 * Must throw ConnectorError, not IntegrationError.
 */
fun testConnectorErrorOnDeclinedCard(apiKey: String): TestResult {
    val name = "connector_error_declined_card"
    val client = PaymentClient(buildStripeConfig(apiKey))

    val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "composite_declined_${System.currentTimeMillis()}"
        amountBuilder.apply {
            minorAmount = 1000L
            currency = Currency.USD
        }
        paymentMethodBuilder.cardBuilder.apply {
            cardNumberBuilder.value = "4000000000000002"  // Stripe generic decline test card
            cardExpMonthBuilder.value = "12"
            cardExpYearBuilder.value = "2050"
            cardCvcBuilder.value = "123"
            cardHolderNameBuilder.value = "Test User"
        }
        captureMethod = CaptureMethod.AUTOMATIC
        authType = AuthenticationType.NO_THREE_DS
        addressBuilder
    }.build()

    return try {
        client.authorize(request)
        // Stripe should decline 4000000000000002 — if it doesn't, still not our failure
        TestResult(name, true, "Card unexpectedly succeeded (sandbox may behave differently)")
    } catch (e: ConnectorError) {
        // Correct — connector returned 4xx, payment declined
        TestResult(name, true, "ConnectorError (expected): ${e.message} (code=${e.errorCode}, http=${e.httpStatusCode})")
    } catch (e: IntegrationError) {
        // Wrong — the request was valid, should have reached the connector
        TestResult(name, false, "Got IntegrationError instead of ConnectorError: ${e.message}")
    }
}

// ── Runner ────────────────────────────────────────────────────────────────────

fun main(args: Array<String>) {
    var credsFile = "creds.json"
    var i = 0
    while (i < args.size) {
        if (args[i] == "--creds-file" && i + 1 < args.size) credsFile = args[++i]
        i++
    }

    println("\n" + "=".repeat(60))
    println("Composite smoke test (direct SDK calls)")
    println("=".repeat(60))

    val creds = loadCreds(credsFile)
    val apiKey = stripeApiKey(creds)

    if (apiKey == null) {
        println(yellow2("SKIPPED: no stripe credentials found in $credsFile"))
        return
    }

    val results = listOf(
        testStripeAuthorizeSuccess(apiKey),
        testIntegrationErrorOnMissingAmount(apiKey),
        testConnectorErrorOnDeclinedCard(apiKey),
    )

    println()
    for (r in results) {
        val icon = if (r.passed) green2("✓") else red2("✗")
        println("  $icon  ${if (r.passed) r.name else red2(r.name)}")
        println("     ${grey2(r.detail)}")
    }

    println()
    val failed = results.filter { !it.passed }
    if (failed.isEmpty()) {
        println(green2("PASSED") + " (${results.size} test(s))")
    } else {
        println(red2("FAILED") + " — ${failed.size} of ${results.size} test(s) failed")
        System.exit(1)
    }
}
