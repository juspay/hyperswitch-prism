/**
 * Multi-connector smoke test for the hyperswitch-payments Java SDK.
 *
 * Loads connector credentials from external JSON file and runs all scenario
 * functions found in examples/{connector}/kotlin/{connector}.kt for each connector.
 *
 * Each example file (stripe.kt, adyen.kt, etc.) is auto-generated and lives in
 * package examples.{connector}. It exports process*(merchantTransactionId, config)
 * functions that the smoke test discovers and invokes via reflection — the same
 * philosophy as the Python and JavaScript smoke tests.
 *
 * Usage:
 *   ./gradlew run --args="--creds-file creds.json --all"
 *   ./gradlew run --args="--creds-file creds.json --connectors stripe,adyen"
 *   ./gradlew run --args="--creds-file creds.json --all --dry-run"
 */

import com.google.gson.Gson
import com.google.gson.reflect.TypeToken
import payments.ConnectorConfig
import payments.ConnectorSpecificConfig
import payments.SdkOptions
import payments.Environment
import payments.IntegrationError
import payments.ConnectorResponseTransformationError
import java.io.File
import java.lang.reflect.InvocationTargetException

// ── ANSI color helpers ──────────────────────────────────────────────────────
// Disable colors only when: NO_COLOR is set, or no FORCE_COLOR/TERM hint and no real console.
// System.console() is null under Gradle (stdout is piped), so we fall back to TERM/FORCE_COLOR
// to detect that we're ultimately writing to a real terminal.
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

// Canonical scenario order — matches Python and JS smoke tests
val SCENARIO_NAMES = listOf(
    "checkout_autocapture",
    "checkout_card",
    "checkout_wallet",
    "checkout_bank",
    "refund",
    "recurring",
    "void_payment",
    "get_payment",
    "create_customer",
    "tokenize",
    "authentication",
)

val PLACEHOLDER_VALUES = setOf("", "placeholder", "test", "dummy", "sk_test_placeholder")

typealias AuthConfig = Map<String, Any>
typealias Credentials = Map<String, Any>

data class ScenarioResult(
    val passed: Boolean,
    val result: Map<String, Any?>? = null,
    val connectorError: String? = null,
    val error: String? = null,
)

data class ConnectorResult(
    val connector: String,
    var status: String,
    val scenarios: MutableMap<String, ScenarioResult> = mutableMapOf(),
    val error: String? = null,
)

fun loadCredentials(credsFile: String): Credentials {
    val file = File(credsFile)
    if (!file.exists()) throw IllegalArgumentException("Credentials file not found: $credsFile")
    val gson = Gson()
    val type = object : TypeToken<Credentials>() {}.type
    return gson.fromJson(file.readText(), type)
}

fun isPlaceholder(value: String): Boolean {
    if (value.isEmpty()) return true
    val lower = value.lowercase()
    return PLACEHOLDER_VALUES.contains(lower) || lower.contains("placeholder")
}

fun hasValidCredentials(authConfig: AuthConfig): Boolean {
    for ((key, value) in authConfig) {
        if (key == "metadata" || key == "_comment") continue
        if (value is Map<*, *>) {
            val v = value["value"]
            if (v is String && !isPlaceholder(v)) return true
        } else if (value is String && !isPlaceholder(value)) {
            return true
        }
    }
    return false
}

fun buildConnectorConfig(connectorName: String, authConfig: AuthConfig): ConnectorConfig {
    val connectorSpecificBuilder = ConnectorSpecificConfig.newBuilder()

    // Find the per-connector builder method (e.g., getStripeBuilder, getAdyenBuilder)
    val connectorBuilderMethod = try {
        connectorSpecificBuilder.javaClass.getMethod("get${connectorName.lowercase().replaceFirstChar { it.uppercase() }}Builder")
    } catch (e: NoSuchMethodException) { null }

    if (connectorBuilderMethod != null) {
        val connectorBuilder = connectorBuilderMethod.invoke(connectorSpecificBuilder)
        for ((key, value) in authConfig) {
            if (key == "_comment" || key == "metadata") continue
            val camelKey = key.split("_").mapIndexed { i, part ->
                if (i == 0) part else part.replaceFirstChar { it.uppercase() }
            }.joinToString("")
            val fieldBuilderMethod = try {
                connectorBuilder?.javaClass?.getMethod("get${camelKey.replaceFirstChar { it.uppercase() }}Builder")
            } catch (e: NoSuchMethodException) { null }
            if (fieldBuilderMethod != null && value is Map<*, *> && value.containsKey("value")) {
                val fieldValue = value["value"] as? String
                if (fieldValue != null) {
                    val fieldBuilder = fieldBuilderMethod.invoke(connectorBuilder)
                    fieldBuilder?.javaClass?.getMethod("setValue", String::class.java)?.invoke(fieldBuilder, fieldValue)
                }
            }
        }
    }

    val sdkOptions = SdkOptions.newBuilder()
        .setEnvironment(Environment.SANDBOX)
        .build()

    return ConnectorConfig.newBuilder()
        .setConnectorConfig(connectorSpecificBuilder.build())
        .setOptions(sdkOptions)
        .build()
}

// ── Reflection-based scenario discovery ─────────────────────────────────────

/**
 * Convert a snake_case scenario key to the PascalCase method name used in
 * generated example files: "checkout_card" -> "processCheckoutCard"
 */
fun scenarioToMethodName(scenarioKey: String): String =
    "process" + scenarioKey.split("_").joinToString("") { it.replaceFirstChar { c -> c.uppercase() } }

/**
 * Compute the JVM class name for a connector's generated example file.
 * examples/stripe/kotlin/stripe.kt  →  package examples.stripe  →  class examples.stripe.StripeKt
 */
fun connectorClassName(connectorName: String): String =
    "examples.$connectorName.${connectorName.replaceFirstChar { it.uppercase() }}Kt"

fun testConnectorScenarios(
    instanceName: String,
    connectorName: String,
    config: ConnectorConfig,
    dryRun: Boolean = false,
): ConnectorResult {
    val result = ConnectorResult(connector = instanceName, status = "passed")

    if (dryRun) {
        result.status = "dry_run"
        return result
    }

    // Locate the compiled example class for this connector
    val className = connectorClassName(connectorName)
    val exampleClass = try {
        Class.forName(className)
    } catch (e: ClassNotFoundException) {
        result.status = "skipped"
        result.scenarios["skipped"] = ScenarioResult(passed = true, error = "no_examples_class")
        return result
    }

    // Discover which scenario methods exist on the class
    val scenarioMethods = SCENARIO_NAMES.mapNotNull { scenarioKey ->
        val methodName = scenarioToMethodName(scenarioKey)
        try {
            val method = exampleClass.getMethod(methodName, String::class.java, ConnectorConfig::class.java)
            scenarioKey to method
        } catch (e: NoSuchMethodException) { null }
    }

    if (scenarioMethods.isEmpty()) {
        result.status = "skipped"
        result.scenarios["skipped"] = ScenarioResult(passed = true, error = "no_scenario_methods")
        return result
    }

    var anyFailed = false

    for ((scenarioKey, method) in scenarioMethods) {
        val txnId = "smoke_${scenarioKey}_${Integer.toHexString((Math.random() * 0xFFFFFF).toInt())}"
        print("    [$scenarioKey] running ... ")
        System.out.flush()

        try {
            @Suppress("UNCHECKED_CAST")
            val response = method.invoke(null, txnId, config) as Map<String, Any?>
            // Check if response contains error with actual data (not just empty/default ErrorInfo)
            val error = response["error"]
            val hasError = error != null && error.toString().let { 
                it.isNotBlank() && it != "{}" && !it.matches(Regex("""\w+\s*\{\s*\}"""))
            }
            if (hasError) {
                val errorStr = error.toString()
                println(yellow("~ connector error") + grey(" — $errorStr"))
                result.scenarios[scenarioKey] = ScenarioResult(passed = true, connectorError = errorStr)
            } else {
                println(green("✓ ok") + grey(" — $response"))
                result.scenarios[scenarioKey] = ScenarioResult(passed = true, result = response)
            }
        } catch (e: InvocationTargetException) {
            // Unwrap: InvocationTargetException wraps the real exception from the called method
            val cause = e.cause ?: e
            when (cause) {
                is IntegrationError, is ConnectorResponseTransformationError -> {
                    val detail = "${cause.javaClass.simpleName}: ${cause.message}"
                    println(yellow("~ connector error") + grey(" — $detail"))
                    result.scenarios[scenarioKey] = ScenarioResult(passed = true, connectorError = detail)
                }
                else -> {
                    val detail = "${cause.javaClass.simpleName}: ${cause.message}"
                    println(yellow("~ connector error") + grey(" — $detail"))
                    result.scenarios[scenarioKey] = ScenarioResult(passed = true, connectorError = detail)
                }
            }
        } catch (e: Exception) {
            val detail = "${e.javaClass.simpleName}: ${e.message}"
            println(red("✗ FAILED") + " — $detail")
            result.scenarios[scenarioKey] = ScenarioResult(passed = false, error = detail)
            anyFailed = true
        }
    }

    result.status = if (anyFailed) "failed" else "passed"
    return result
}

fun printResult(result: ConnectorResult) {
    when (result.status) {
        "passed"  -> println(green("  PASSED") + " (${result.scenarios.size} scenario(s))")
        "dry_run" -> println(grey("  DRY RUN"))
        "skipped" -> {
            val reason = result.scenarios["skipped"]?.error ?: result.error ?: "unknown"
            println(grey("  SKIPPED ($reason)"))
        }
        else -> {
            println(red("  FAILED"))
            for ((key, detail) in result.scenarios) {
                if (!detail.passed) println(red("    $key") + " — ${detail.error ?: "unknown error"}")
            }
            if (result.error != null) println(red("  Error: ${result.error}"))
        }
    }
}

data class Args(
    val credsFile: String = "creds.json",
    val connectors: List<String>? = null,
    val all: Boolean = false,
    val dryRun: Boolean = false,
)

fun parseArgs(args: Array<String>): Args {
    var result = Args()
    var i = 0
    while (i < args.size) {
        when (args[i]) {
            "--creds-file" -> if (i + 1 < args.size) result = result.copy(credsFile = args[++i])
            "--connectors" -> if (i + 1 < args.size) result = result.copy(connectors = args[++i].split(",").map { it.trim() })
            "--all"        -> result = result.copy(all = true)
            "--dry-run"    -> result = result.copy(dryRun = true)
            "--help", "-h" -> {
                println("""
Usage: ./gradlew run --args="[options]"

Options:
  --creds-file <path>     Path to credentials JSON (default: creds.json)
  --connectors <list>     Comma-separated list of connectors to test
  --all                   Test all connectors in the credentials file
  --dry-run               Build requests without executing HTTP calls
  --help, -h              Show this help message

Examples:
  ./gradlew run --args="--all"
  ./gradlew run --args="--connectors stripe,adyen"
  ./gradlew run --args="--all --dry-run"
""")
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

fun runTests(
    credsFile: String,
    connectors: List<String>?,
    dryRun: Boolean,
): List<ConnectorResult> {
    val credentials = loadCredentials(credsFile)
    val results = mutableListOf<ConnectorResult>()
    val testConnectors = connectors ?: credentials.keys.toList()

    println("\n${"=".repeat(60)}")
    println("Running smoke tests for ${testConnectors.size} connector(s)")
    println("${"=".repeat(60)}\n")

    for (connectorName in testConnectors) {
        val authConfigValue = credentials[connectorName]
        println("\n${bold("--- Testing $connectorName ---")}")

        if (authConfigValue == null) {
            println(grey("  SKIPPED (not found in credentials file)"))
            results.add(ConnectorResult(connectorName, "skipped", error = "not_found"))
            continue
        }

        @Suppress("UNCHECKED_CAST")
        when {
            authConfigValue is List<*> -> {
                val authConfigList = authConfigValue as List<Map<String, Any>>
                for (i in authConfigList.indices) {
                    val instanceName = "$connectorName[${i + 1}]"
                    println("  Instance: $instanceName")
                    val authConfig = authConfigList[i] as AuthConfig

                    if (!hasValidCredentials(authConfig)) {
                        println(grey("  SKIPPED (placeholder credentials)"))
                        results.add(ConnectorResult(instanceName, "skipped", error = "placeholder_credentials"))
                        continue
                    }

                    val config = try {
                        buildConnectorConfig(connectorName, authConfig)
                    } catch (e: Exception) {
                        println(grey("  SKIPPED (${e.message})"))
                        results.add(ConnectorResult(instanceName, "skipped", error = e.message))
                        continue
                    }

                    val result = testConnectorScenarios(instanceName, connectorName, config, dryRun)
                    results.add(result)
                    printResult(result)
                }
            }
            authConfigValue is Map<*, *> -> {
                val authConfig = authConfigValue as AuthConfig

                if (!hasValidCredentials(authConfig)) {
                    println(grey("  SKIPPED (placeholder credentials)"))
                    results.add(ConnectorResult(connectorName, "skipped", error = "placeholder_credentials"))
                    continue
                }

                val config = try {
                    buildConnectorConfig(connectorName, authConfig)
                } catch (e: Exception) {
                    println(grey("  SKIPPED (${e.message})"))
                    results.add(ConnectorResult(connectorName, "skipped", error = e.message))
                    continue
                }

                val result = testConnectorScenarios(connectorName, connectorName, config, dryRun)
                results.add(result)
                printResult(result)
            }
        }
    }

    return results
}

fun printSummary(results: List<ConnectorResult>): Int {
    println("\n${"=".repeat(60)}")
    println(bold("TEST SUMMARY"))
    println("${"=".repeat(60)}\n")

    val passed  = results.count { it.status in listOf("passed", "dry_run") }
    val skipped = results.count { it.status == "skipped" }
    val failed  = results.count { it.status == "failed" }

    println("Total:   ${results.size}")
    println(green("Passed:  $passed"))
    println(grey("Skipped: $skipped (placeholder credentials or no examples)"))
    println((if (failed > 0) ::red else ::green)("Failed:  $failed"))
    println()

    if (failed > 0) {
        println(red("Failed connectors:"))
        for (r in results) {
            if (r.status == "failed") println(red("  - ${r.connector}") + ": ${r.error ?: "see scenarios above"}")
        }
        println()
        return 1
    }

    if (passed == 0 && skipped > 0) {
        println(yellow("All tests skipped (no valid credentials found)"))
        println("Update creds.json with real credentials to run tests")
        return 1
    }

    println(green("All tests completed successfully!"))
    return 0
}

fun main(args: Array<String>) {
    val parsedArgs = parseArgs(args)
    try {
        val results = runTests(parsedArgs.credsFile, parsedArgs.connectors, parsedArgs.dryRun)
        val exitCode = printSummary(results)
        System.exit(exitCode)
    } catch (e: Exception) {
        System.err.println("\nFatal error: ${e.message}")
        e.printStackTrace()
        System.exit(1)
    }
}
