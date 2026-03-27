import kotlinx.coroutines.runBlocking

/**
 * gRPC smoke test for the hyperswitch-payments Kotlin SDK.
 */

import com.google.gson.Gson
import com.google.gson.reflect.TypeToken
import payments.GrpcClient
import payments.GrpcConfig
import java.io.File
import types.Payment.PaymentServiceAuthorizeRequest
import types.Payment.PaymentServiceCaptureRequest
import types.Payment.PaymentServiceGetRequest
import types.Payment.PaymentServiceRefundRequest
import types.Payment.CaptureMethod
import types.Payment.Currency
import types.Payment.AuthenticationType

// ANSI color helpers
private val NO_COLOR = System.getenv("NO_COLOR") != null
    || (System.getenv("FORCE_COLOR") == null
    && System.console() == null
    && System.getenv("TERM").let { it == null || it == "dumb" })

private fun c(code: String, text: String) = if (NO_COLOR) text else "\u001b[${code}m$text\u001b[0m"
private fun green (t: String) = c("32", t)
private fun yellow(t: String) = c("33", t)
private fun red   (t: String) = c("31", t)
private fun grey  (t: String) = c("90", t)
private fun bold  (t: String) = c("1",  t)

// Request Builders
fun buildAuthorizeRequest(captureMethod: CaptureMethod): PaymentServiceAuthorizeRequest {
    return PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"
        amountBuilder.apply {
            minorAmount = 1000L
            currency = Currency.USD
        }
        paymentMethodBuilder.apply {
            cardBuilder.apply {
                cardNumberBuilder.value = "4111111111111111"
                cardExpMonthBuilder.value = "03"
                cardExpYearBuilder.value = "2030"
                cardCvcBuilder.value = "737"
                cardHolderNameBuilder.value = "John Doe"
            }
        }
        addressBuilder.billingAddressBuilder  // initialize empty billing address (required by connector-service)
        this.captureMethod = captureMethod
        authType = AuthenticationType.NO_THREE_DS
        returnUrl = "https://example.com/return"
    }.build()
}

fun buildCaptureRequest(txnId: String): PaymentServiceCaptureRequest {
    return PaymentServiceCaptureRequest.newBuilder().apply {
        merchantCaptureId = "probe_capture_001"
        connectorTransactionId = txnId
        amountToCaptureBuilder.apply {
            minorAmount = 1000L
            currency = Currency.USD
        }
    }.build()
}

fun buildGetRequest(txnId: String): PaymentServiceGetRequest {
    return PaymentServiceGetRequest.newBuilder().apply {
        merchantTransactionId = "probe_merchant_txn_001"
        connectorTransactionId = txnId
        amountBuilder.apply {
            minorAmount = 1000L
            currency = Currency.USD
        }
    }.build()
}

fun buildRefundRequest(txnId: String): PaymentServiceRefundRequest {
    return PaymentServiceRefundRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"
        connectorTransactionId = txnId
        paymentAmount = 1000L
        refundAmountBuilder.apply {
            minorAmount = 1000L
            currency = Currency.USD
        }
        reason = "customer_request"
    }.build()
}

fun buildVoidRequest(txnId: String): types.Payment.PaymentServiceVoidRequest {
    return types.Payment.PaymentServiceVoidRequest.newBuilder().apply {
        merchantVoidId = "probe_void_001"
        connectorTransactionId = txnId
    }.build()
}

// Credentials helpers
fun credStr(cred: AuthConfig, vararg keys: String): String? {
    for (key in keys) {
        val value = cred[key]
        if (value is String && value.isNotEmpty()) return value
        if (value is Map<*, *>) {
            val inner = value["value"]
            if (inner is String && inner.isNotEmpty()) return inner
        }
    }
    return null
}

fun buildGrpcConfig(connector: String, cred: AuthConfig): GrpcConfig {
    val connectorVariant = connector.replaceFirstChar { it.uppercase() }
    val apiKey = credStr(cred, "api_key", "apiKey") ?: "placeholder"
    val apiSecret = credStr(cred, "api_secret", "apiSecret")
    val key1 = credStr(cred, "key1")
    val merchantId = credStr(cred, "merchant_id", "merchantId")
    val tenantId = credStr(cred, "tenant_id", "tenantId")
    
    val connectorSpecificConfig = mutableMapOf<String, Any>("api_key" to apiKey)
    if (apiSecret != null) connectorSpecificConfig["api_secret"] = apiSecret
    if (key1 != null) connectorSpecificConfig["key1"] = key1
    if (merchantId != null) connectorSpecificConfig["merchant_id"] = merchantId
    if (tenantId != null) connectorSpecificConfig["tenant_id"] = tenantId
    
    val connectorConfig: Map<String, Any> = mapOf(
        "config" to mapOf(connectorVariant to connectorSpecificConfig)
    )
    
    return GrpcConfig(
        endpoint = credStr(cred, "endpoint") ?: "http://localhost:8000",
        connector = connector,
        connectorConfig = connectorConfig,
    )
}

// Args parsing
data class GrpcArgs(
    val credsFile: String = "creds.json",
    val connectors: List<String>? = null,
    val all: Boolean = false,
)

fun parseGrpcArgs(args: Array<String>): GrpcArgs {
    var result = GrpcArgs()
    var i = 0
    while (i < args.size) {
        when (args[i]) {
            "--grpc" -> { }
            "--creds-file" -> if (i + 1 < args.size) result = result.copy(credsFile = args[++i])
            "--connectors" -> if (i + 1 < args.size) result = result.copy(connectors = args[++i].split(",").map { it.trim() })
            "--all" -> result = result.copy(all = true)
            "--help", "-h" -> {
                println("Usage: ./gradlew runGrpc --args=\"[options]\"")
                println("Options:")
                println("  --creds-file <path>     Path to credentials JSON")
                println("  --connectors <list>     Comma-separated list of connectors")
                println("  --all                   Test all connectors")
                println("  --help, -h              Show this help")
                System.exit(0)
            }
        }
        i++
    }
    if (!result.all && result.connectors == null) {
        System.err.println("Error: Must specify either --all or --connectors")
        System.exit(1)
    }
    return result
}

// Main test function
fun main(args: Array<String>) {
    val parsedArgs = parseGrpcArgs(args)
    
    val exitCode = runBlocking {
        runGrpcTests(parsedArgs.credsFile, parsedArgs.connectors)
    }
    
    System.exit(exitCode)
}

suspend fun runGrpcTests(credsFile: String, connectors: List<String>?): Int {
    val credentials = loadCredentials(credsFile)
    val testConnectors = connectors ?: credentials.keys.toList()
    
    println("\n${"=".repeat(60)}")
    println(bold("hyperswitch gRPC smoke test (Kotlin)"))
    println(grey("connectors: ${testConnectors.joinToString(", ")}"))
    println("${"=".repeat(60)}\n")
    
    var anyFailed = false
    
    for (connectorName in testConnectors) {
        println(bold("── $connectorName ──"))
        
        val authConfigValue = credentials[connectorName]
        if (authConfigValue == null) {
            println(grey("  [$connectorName] not found in credentials file, skipping."))
            continue
        }
        
        val authConfigList: List<AuthConfig> = when (authConfigValue) {
            is List<*> -> authConfigValue as List<AuthConfig>
            is Map<*, *> -> listOf(authConfigValue as AuthConfig)
            else -> {
                println(grey("  [$connectorName] invalid credentials format, skipping."))
                continue
            }
        }
        
        for (authConfig in authConfigList) {
            val config = buildGrpcConfig(connectorName, authConfig)
            val client = GrpcClient(config)
            
            var txnId = "probe_connector_txn_001"
            
            // Authorize
            print("  [authorize] running … ")
            try {
                val req = buildAuthorizeRequest(CaptureMethod.AUTOMATIC)
                val res = client.direct_payment.authorize(req)
                txnId = res.connectorTransactionId ?: txnId
                val msg = "txn_id: $txnId, status: ${res.statusCode}"
                if (res.statusCode >= 400) {
                    println("${yellow("~ connector error")} ${grey("— $msg")}")
                } else {
                    println("${green("✓ ok")} ${grey("— $msg")}")
                }
            } catch (e: Exception) {
                val err = e.message ?: "unknown error"
                val isTransport = err.contains("unavailable", ignoreCase = true) ||
                    err.contains("deadlineexceeded", ignoreCase = true) ||
                    err.contains("connection refused", ignoreCase = true) ||
                    err.contains("transport error", ignoreCase = true)
                if (isTransport) {
                    println("${red("✗ FAILED")} ${grey("— $err")}")
                    anyFailed = true
                } else {
                    println("${yellow("~ connector error")} ${grey("— $err")}")
                }
            }
            
            // Capture
            print("  [capture] running … ")
            try {
                val authReq = buildAuthorizeRequest(CaptureMethod.MANUAL)
                val authRes = client.direct_payment.authorize(authReq)
                if (authRes.statusCode >= 400) {
                    throw RuntimeException("inline authorize failed (status ${authRes.statusCode})")
                }
                val captureTxnId = authRes.connectorTransactionId ?: txnId
                val capReq = buildCaptureRequest(captureTxnId)
                val capRes = client.direct_payment.capture(capReq)
                val msg = "txn_id: ${capRes.connectorTransactionId ?: "-"}, status: ${capRes.statusCode}"
                if (capRes.statusCode >= 400) {
                    println("${yellow("~ connector error")} ${grey("— $msg")}")
                } else {
                    println("${green("✓ ok")} ${grey("— $msg")}")
                }
            } catch (e: Exception) {
                val err = e.message ?: "unknown error"
                val isTransport = err.contains("unavailable", ignoreCase = true) ||
                    err.contains("deadlineexceeded", ignoreCase = true) ||
                    err.contains("connection refused", ignoreCase = true) ||
                    err.contains("transport error", ignoreCase = true)
                if (isTransport) {
                    println("${red("✗ FAILED")} ${grey("— $err")}")
                    anyFailed = true
                } else {
                    println("${yellow("~ connector error")} ${grey("— $err")}")
                }
            }
            
            // Void
            print("  [void] running … ")
            try {
                val authReq = buildAuthorizeRequest(CaptureMethod.MANUAL)
                val authRes = client.direct_payment.authorize(authReq)
                if (authRes.statusCode >= 400) {
                    throw RuntimeException("inline authorize failed (status ${authRes.statusCode})")
                }
                val voidTxnId = authRes.connectorTransactionId ?: txnId
                val voidReq = buildVoidRequest(voidTxnId)
                val voidRes = client.direct_payment.void(voidReq)
                val msg = "void_id: ${voidRes.merchantVoidId ?: "-"}, status: ${voidRes.statusCode}"
                if (voidRes.statusCode >= 400) {
                    println("${yellow("~ connector error")} ${grey("— $msg")}")
                } else {
                    println("${green("✓ ok")} ${grey("— $msg")}")
                }
            } catch (e: Exception) {
                val err = e.message ?: "unknown error"
                val isTransport = err.contains("unavailable", ignoreCase = true) ||
                    err.contains("deadlineexceeded", ignoreCase = true) ||
                    err.contains("connection refused", ignoreCase = true) ||
                    err.contains("transport error", ignoreCase = true)
                if (isTransport) {
                    println("${red("✗ FAILED")} ${grey("— $err")}")
                    anyFailed = true
                } else {
                    println("${yellow("~ connector error")} ${grey("— $err")}")
                }
            }
            
            // Get
            print("  [get] running … ")
            try {
                val getReq = buildGetRequest(txnId)
                val getRes = client.direct_payment.get(getReq)
                val msg = "txn_id: ${getRes.connectorTransactionId ?: "-"}, status: ${getRes.statusCode}"
                if (getRes.statusCode >= 400) {
                    println("${yellow("~ connector error")} ${grey("— $msg")}")
                } else {
                    println("${green("✓ ok")} ${grey("— $msg")}")
                }
            } catch (e: Exception) {
                val err = e.message ?: "unknown error"
                val isTransport = err.contains("unavailable", ignoreCase = true) ||
                    err.contains("deadlineexceeded", ignoreCase = true) ||
                    err.contains("connection refused", ignoreCase = true) ||
                    err.contains("transport error", ignoreCase = true)
                if (isTransport) {
                    println("${red("✗ FAILED")} ${grey("— $err")}")
                    anyFailed = true
                } else {
                    println("${yellow("~ connector error")} ${grey("— $err")}")
                }
            }
            
            // Refund
            print("  [refund] running … ")
            try {
                val refReq = buildRefundRequest(txnId)
                val refRes = client.direct_payment.refund(refReq)
                val msg = "refund_id: ${refRes.connectorRefundId ?: "-"}, status: ${refRes.statusCode}"
                if (refRes.statusCode >= 400) {
                    println("${yellow("~ connector error")} ${grey("— $msg")}")
                } else {
                    println("${green("✓ ok")} ${grey("— $msg")}")
                }
            } catch (e: Exception) {
                val err = e.message ?: "unknown error"
                val isTransport = err.contains("unavailable", ignoreCase = true) ||
                    err.contains("deadlineexceeded", ignoreCase = true) ||
                    err.contains("connection refused", ignoreCase = true) ||
                    err.contains("transport error", ignoreCase = true)
                if (isTransport) {
                    println("${red("✗ FAILED")} ${grey("— $err")}")
                    anyFailed = true
                } else {
                    println("${yellow("~ connector error")} ${grey("— $err")}")
                }
            }
        }
        println()
    }
    
    println(if (anyFailed) red("Some gRPC tests FAILED.") else green("All gRPC tests passed."))
    return if (anyFailed) 1 else 0
}
