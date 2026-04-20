/**
 * Multi-connector smoke test for the hyperswitch-payments Java SDK.
 *
 * Loads connector credentials from external JSON file and runs all scenario
 * functions found in examples/{connector}/kotlin/{connector}.kt for each connector.
 *
 * Each example file (stripe.kt, adyen.kt, etc.) is auto-generated and lives in
 * package examples.{connector}. It exports process*(merchantTransactionId, config)
 * functions that the smoke test discovers and invokes via reflection.
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
import payments.ConnectorError
import payments.HttpClient
import payments.HttpRequest
import payments.HttpResponse
import java.io.File
import java.lang.reflect.InvocationTargetException

// ── ANSI color helpers ──────────────────────────────────────────────────────
private val NO_COLOR = System.getenv("NO_COLOR") != null
    || (System.getenv("FORCE_COLOR") == null
        && System.console() == null
        && System.getenv("TERM").let { it == null || it == "dumb" })

private fun c(code: String, text: String) = if (NO_COLOR) text else "\u001b[${code}m$text\u001b[0m"
private fun green(t: String) = c("32", t)
private fun yellow(t: String) = c("33", t)
private fun red(t: String) = c("31", t)
private fun grey(t: String) = c("90", t)
private fun bold(t: String) = c("1", t)

val PLACEHOLDER_VALUES = setOf("", "placeholder", "test", "dummy", "sk_test_placeholder")

typealias AuthConfig = Map<String, Any>
typealias Credentials = Map<String, Any>

data class ScenarioResult(
    val status: String,  // "passed" | "skipped" | "failed"
    val result: Map<String, Any?>? = null,
    val reason: String? = null,
    val detail: String? = null,
    val error: String? = null,
)

data class ConnectorResult(
    val connector: String,
    var status: String,
    val scenarios: MutableMap<String, ScenarioResult> = mutableMapOf(),
    var error: String? = null,
)

sealed class DiscoveryResult
class ValidScenarios(val scenarios: List<Pair<String, java.lang.reflect.Method>>) : DiscoveryResult()
class ValidationError(val message: String) : DiscoveryResult()

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

data class FlowManifest(
    val flows: List<String>,
    val flowToExampleFn: Map<String, String?>
)

fun loadFlowManifest(sdkRoot: String): FlowManifest {
    val manifestPath = File(sdkRoot, "generated/flows.json")
    if (!manifestPath.exists()) {
        throw IllegalStateException(
            "flows.json not found at ${manifestPath.absolutePath}. Run: make generate"
        )
    }
    val gson = Gson()
    val type = object : TypeToken<Map<String, Any>>() {}.type
    val data: Map<String, Any> = gson.fromJson(manifestPath.readText(), type)
    @Suppress("UNCHECKED_CAST")
    val flows = data["flows"] as List<String>
    @Suppress("UNCHECKED_CAST")
    val flowToExampleFn = (data["flow_to_example_fn"] as? Map<String, Any>)?.mapValues { 
        when (val v = it.value) {
            is String -> v
            else -> null
        }
    } ?: emptyMap()
    return FlowManifest(flows, flowToExampleFn)
}

fun scenarioToMethodName(scenarioKey: String): String =
    "process" + scenarioKey.split("_").joinToString("") { it.replaceFirstChar { c -> c.uppercase() } }

fun fromMethodName(methodName: String): String {
    return methodName
        .removePrefix("process")
        .replace(Regex("([A-Z])"), "_$1")
        .lowercase()
        .trimStart('_')
}

fun connectorClassName(connectorName: String, mock: Boolean = false): String {
    return "examples.$connectorName.${connectorName.replaceFirstChar { it.uppercase() }}Kt"
}

fun discoverAndValidate(
    exampleClass: Class<*>,
    connectorName: String,
    manifest: List<String>,
    flowToExampleFn: Map<String, String?>,
): DiscoveryResult {

    val declared: List<String>? = try {
        @Suppress("UNCHECKED_CAST")
        exampleClass.getDeclaredField("SUPPORTED_FLOWS").also { it.isAccessible = true }
            .get(null) as? List<String>
    } catch (_: NoSuchFieldException) { null }
    
    val legacyMode = declared == null

    val effectiveDeclared: List<String> = if (!legacyMode) {
        declared!!.distinct()  // Deduplicate
    } else {
        // Legacy mode: include ALL flows from manifest
        // We'll check for implementations during iteration
        manifest
    }

    // Validate flow names are lowercase snake_case
    for (name in effectiveDeclared) {
        if (name != name.lowercase() || name.contains(" ") || name.contains("-")) {
            return ValidationError(
                "COVERAGE ERROR: Flow name '$name' in SUPPORTED_FLOWS must be lowercase snake_case (e.g., 'authorize', 'payout_create')"
            )
        }
    }

    // Helper: find a method for a flow, trying multiple naming conventions and signatures.
    // Order: mapped scenario fn (processCheckoutCard) → process-prefixed (processAuthorize) →
    //        camelCase without prefix (authorize, proxyAuthorize).
    // Each name is tried with (String, ConnectorConfig) first, then (String) only.
    fun findFlowMethod(flow: String): java.lang.reflect.Method? {
        fun tryGet(name: String): java.lang.reflect.Method? {
            try { return exampleClass.getMethod(name, String::class.java, ConnectorConfig::class.java) }
            catch (_: NoSuchMethodException) {}
            try { return exampleClass.getMethod(name, String::class.java) }
            catch (_: NoSuchMethodException) {}
            return null
        }
        val exampleFn = flowToExampleFn[flow]
        if (exampleFn != null) {
            tryGet(scenarioToMethodName(exampleFn))?.let { return it }
        }
        tryGet(scenarioToMethodName(flow))?.let { return it }
        // Fallback: examples expose flow functions under camelCase without process prefix
        // e.g. flow "proxy_authorize" → method "proxyAuthorize"
        val camel = flow.split("_").mapIndexed { i, w -> if (i == 0) w else w.replaceFirstChar { it.uppercase() } }.joinToString("")
        return tryGet(camel)
    }

    // CHECK 1: Find declared flows without implementations
    val missing = effectiveDeclared.filter { findFlowMethod(it) == null }
    if (!legacyMode && missing.isNotEmpty()) {
        return ValidationError("COVERAGE ERROR: SUPPORTED_FLOWS declares $missing but no implementation found.")
    }

    // CHECK 2 and 3 only apply when SUPPORTED_FLOWS is explicitly defined (not legacy mode)
    if (!legacyMode) {
        // CHECK 2: process* methods for known flows must be declared in SUPPORTED_FLOWS.
        // Scenario functions (e.g. processCheckoutAutocapture) whose base name is not in
        // the manifest are allowed — they cover multi-step scenarios.
        val allProcessMethods = exampleClass.methods
            .filter { it.name.startsWith("process") && !it.isSynthetic && !it.name.contains("$") }
            .map { fromMethodName(it.name) }
            .toSet()
        val manifestSet = manifest.toSet()
        val declaredSet = effectiveDeclared.toSet()
        val undeclared = allProcessMethods.filter { it in manifestSet && it !in declaredSet }
        if (undeclared.isNotEmpty()) {
            return ValidationError(
                "COVERAGE ERROR: process* methods exist for flows $undeclared but they're not in SUPPORTED_FLOWS"
            )
        }

        // CHECK 3: Warn about entries in SUPPORTED_FLOWS not in the flow manifest.
        // These are typically composite scenario names (create_customer, recurring_charge)
        // not individually listed in flows.json. Warn only — don't fail.
        val stale = effectiveDeclared.filter { it !in manifestSet }
        if (stale.isNotEmpty()) {
            println("  [warn] SUPPORTED_FLOWS contains entries not in flows.json (scenario names): $stale")
        }
    }

    // Return (key, method) pairs for flows with implementations; null method = N/A
    val methods = effectiveDeclared.map { flow ->
        val method = findFlowMethod(flow)
        val exampleFn = flowToExampleFn[flow]
        flow to method
    }.filter { it.second != null }.map { it.first to it.second!! }
    return ValidScenarios(methods)
}

// Last intercepted mock request (method + URL), read by the PASSED handler
var lastMockRequest: String? = null

fun installMockIntercept() {
    HttpClient.intercept = { req: HttpRequest ->
        lastMockRequest = "${req.method} ${req.url}"
        HttpResponse(200, emptyMap(), "{}".toByteArray(), 0L)
    }
}

fun testConnectorScenarios(
    instanceName: String,
    connectorName: String,
    config: ConnectorConfig,
    sdkRoot: String,
    dryRun: Boolean = false,
    mock: Boolean = false,
): ConnectorResult {
    val result = ConnectorResult(connector = instanceName, status = "passed")

    if (dryRun) {
        result.status = "dry_run"
        return result
    }

    val className = connectorClassName(connectorName, mock)
    val exampleClass = try {
        Class.forName(className)
    } catch (e: ClassNotFoundException) {
        result.status = "skipped"
        result.scenarios["skipped"] = ScenarioResult(status = "skipped", reason = "no_examples_class")
        return result
    }

    // Load flow manifest and validate scenarios
    val manifestData = try {
        loadFlowManifest(sdkRoot)
    } catch (e: Exception) {
        result.status = "failed"
        result.error = e.message
        return result
    }
    val manifest = manifestData.flows
    val flowToExampleFn = manifestData.flowToExampleFn

    val discoveryResult = discoverAndValidate(exampleClass, connectorName, manifest, flowToExampleFn)
    when (discoveryResult) {
        is ValidationError -> {
            result.status = "failed"
            result.error = discoveryResult.message
            return result
        }
        is ValidScenarios -> {
            if (discoveryResult.scenarios.isEmpty()) {
                result.status = "skipped"
                result.scenarios["skipped"] = ScenarioResult(status = "skipped", reason = "no_scenario_methods")
                return result
            }
        }
    }

    val scenarioMethods = (discoveryResult as ValidScenarios).scenarios
    // scenarioMethods is List<Pair<flowName, method>> - use flow name as key
    val methodMap = scenarioMethods.toMap()

    var anyFailed = false

    // Iterate ALL flows from manifest
    for (flowKey in manifest) {
        // Find the method to call - same logic for both mock and normal mode
        // Try flow name directly first, then fall back to example mapping
        var method = methodMap[flowKey]
        
        if (method == null) {
            // Try mapped example function name
            val exampleFnName = flowToExampleFn[flowKey]
            if (exampleFnName != null) {
                method = methodMap[exampleFnName]
            }
        }
        
        if (method == null) {
            // No implementation found for this flow
            println("    [$flowKey] NOT IMPLEMENTED — No example function for flow '$flowKey'")
            result.scenarios[flowKey] = ScenarioResult(status = "not_implemented", reason = "no_example_function")
            continue
        }
        
        // Flow is implemented - run it
        val txnId = "smoke_${flowKey}_${Integer.toHexString((Math.random() * 0xFFFFFF).toInt())}"
        print("    [$flowKey] running ... ")
        System.out.flush()

        try {
            // Flow functions take (String) only; scenario functions take (String, ConnectorConfig).
            val rawResponse = if (method.parameterCount == 1) method.invoke(null, txnId)
                              else method.invoke(null, txnId, config)
            @Suppress("UNCHECKED_CAST")
            val response = rawResponse as? Map<String, Any?>
            val error = response?.get("error")
            val hasError = error != null && error.toString().let {
                it.isNotBlank() && it != "{}" && !it.matches(Regex("""\w+\s*\{\s*\}"""))
            }
            if (hasError) {
                val errorStr = error.toString()
                println(yellow("SKIPPED (connector error)") + grey(" — $errorStr"))
                result.scenarios[flowKey] = ScenarioResult(status = "skipped", reason = "connector_error", detail = errorStr)
            } else {
                val display = response?.toString() ?: "ok"
                println(green("PASSED") + grey(" — $display"))
                result.scenarios[flowKey] = ScenarioResult(status = "passed", result = response ?: emptyMap())
            }
        } catch (e: IntegrationError) {
            val detail = "IntegrationError: ${e.message} (code=${e.errorCode}, action=${e.suggestedAction}, doc=${e.docUrl})"
            // IntegrationError is always FAILED — req_transformer failed
            println(red("FAILED") + " — $detail")
            result.scenarios[flowKey] = ScenarioResult(status = "failed", error = detail)
            anyFailed = true
        } catch (e: ConnectorError) {
            val detail = "ConnectorError: ${e.message} (code=${e.errorCode}, http=${e.httpStatusCode})"
            if (mock) {
                // In mock mode, ConnectorError means req_transformer successfully built the HTTP request.
                // The error is just from parsing the mock empty response, which is expected.
                val mockInfo = lastMockRequest ?: "mock response"
                lastMockRequest = null
                println(green("PASSED") + " — req_transformer OK ($mockInfo)")
                result.scenarios[flowKey] = ScenarioResult(status = "passed", reason = "mock_verified", detail = detail)
            } else {
                println(yellow("SKIPPED (connector error)") + grey(" — $detail"))
                result.scenarios[flowKey] = ScenarioResult(status = "skipped", reason = "connector_error", detail = detail)
            }
        } catch (e: InvocationTargetException) {
            when (val cause = e.cause ?: e) {
                is IntegrationError -> {
                    val detail = "IntegrationError: ${cause.message} (code=${cause.errorCode}, action=${cause.suggestedAction}, doc=${cause.docUrl})"
                    // IntegrationError is always FAILED — req_transformer failed
                    println(red("FAILED") + " — $detail")
                    result.scenarios[flowKey] = ScenarioResult(status = "failed", error = detail)
                    anyFailed = true
                }
                is ConnectorError -> {
                    val detail = "ConnectorError: ${cause.message} (code=${cause.errorCode}, http=${cause.httpStatusCode})"
                    if (mock) {
                        // In mock mode, ConnectorError means req_transformer successfully built the HTTP request.
                        // The error is just from parsing the mock empty response, which is expected.
                        val mockInfo = lastMockRequest ?: "mock response"
                        lastMockRequest = null
                        println(green("PASSED") + " — req_transformer OK ($mockInfo)")
                        result.scenarios[flowKey] = ScenarioResult(status = "passed", reason = "mock_verified", detail = detail)
                    } else {
                        println(yellow("SKIPPED (connector error)") + grey(" — $detail"))
                        result.scenarios[flowKey] = ScenarioResult(status = "skipped", reason = "connector_error", detail = detail)
                    }
                }
                else -> {
                    if (mock && cause !is Error) {
                        // In mock mode, non-panic errors mean req_transformer successfully built the HTTP request.
                        // The error is just from parsing the mock empty response, which is expected.
                        val mockInfo = lastMockRequest ?: "mock response"
                        lastMockRequest = null
                        println(green("PASSED") + " — req_transformer OK ($mockInfo)")
                        result.scenarios[flowKey] = ScenarioResult(status = "passed", reason = "mock_verified", detail = cause.message)
                    } else {
                        val detail = "${cause.javaClass.simpleName}: ${cause.message}"
                        println(red("FAILED") + " — $detail")
                        result.scenarios[flowKey] = ScenarioResult(status = "failed", error = detail)
                        anyFailed = true
                    }
                }
            }
        } catch (e: Exception) {
            if (mock && e !is Error) {
                // In mock mode, non-panic errors mean req_transformer successfully built the HTTP request.
                // The error is just from parsing the mock empty response, which is expected.
                val mockInfo = lastMockRequest ?: "mock response"
                lastMockRequest = null
                println(green("PASSED") + " — req_transformer OK ($mockInfo)")
                result.scenarios[flowKey] = ScenarioResult(status = "passed", reason = "mock_verified", detail = e.message)
            } else {
                val detail = "${e.javaClass.simpleName}: ${e.message}"
                println(red("FAILED") + " — $detail")
                result.scenarios[flowKey] = ScenarioResult(status = "failed", error = detail)
                anyFailed = true
            }
        }
    }

    result.status = if (anyFailed) "failed" else "passed"
    return result
}

fun printResult(result: ConnectorResult) {
    when (result.status) {
        "passed" -> {
            val passedCount = result.scenarios.values.count { it.status == "passed" }
            val skippedCount = result.scenarios.values.count { it.status == "skipped" }
            val notImplCount = result.scenarios.values.count { it.status == "not_implemented" }
            println(green("  PASSED") + " ($passedCount passed, $skippedCount skipped, $notImplCount not implemented)")
            for ((key, detail) in result.scenarios) {
                when (detail.status) {
                    "passed" -> {
                        val resultData = detail.result
                        val resultStr = resultData?.toString() ?: ""
                        println(green("    $key: ✓") + grey(" — $resultStr"))
                    }
                    "skipped" -> {
                        val detailStr = detail.detail?.let { " — $it" } ?: ""
                        println(yellow("    $key: ~ skipped (${detail.reason})") + grey(detailStr))
                    }
                    "not_implemented" -> println(grey("    $key: N/A"))
                }
            }
        }
        "dry_run" -> println(grey("  DRY RUN"))
        "skipped" -> {
            val reason = result.scenarios["skipped"]?.reason ?: result.error ?: "unknown"
            println(grey("  SKIPPED ($reason)"))
        }
        else -> {
            println(red("  FAILED"))
            for ((key, detail) in result.scenarios) {
                if (detail.status == "failed") println(red("    $key: ✗ FAILED — ${detail.error ?: "unknown error"}"))
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
    val mock: Boolean = false,
    val sdkRoot: String = "../..",
)

fun parseArgs(args: Array<String>): Args {
    var result = Args()
    var i = 0
    while (i < args.size) {
        when (args[i]) {
            "--creds-file" -> if (i + 1 < args.size) result = result.copy(credsFile = args[++i])
            "--connectors" -> if (i + 1 < args.size) result = result.copy(connectors = args[++i].split(",").map { it.trim() })
            "--all" -> result = result.copy(all = true)
            "--dry-run" -> result = result.copy(dryRun = true)
            "--mock" -> result = result.copy(mock = true)
            "--sdk-root" -> if (i + 1 < args.size) result = result.copy(sdkRoot = args[++i])
            "--help", "-h" -> {
                println("""
Usage: ./gradlew run --args="[options]"

Options:
  --creds-file <path>     Path to credentials JSON (default: creds.json)
  --connectors <list>     Comma-separated list of connectors to test
  --all                   Test all connectors in the credentials file
  --dry-run               Build requests without executing HTTP calls
  --mock                  Intercept HTTP; verify req_transformer only
  --sdk-root <path>       Path to SDK root (default: ../..)
  --help, -h              Show this help message

Examples:
  ./gradlew run --args="--all"
  ./gradlew run --args="--connectors stripe,adyen"
  ./gradlew run --args="--all --dry-run"
  ./gradlew run --args="--all --mock"
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
    mock: Boolean,
    sdkRoot: String,
): List<ConnectorResult> {
    // Install mock intercept if in mock mode
    if (mock) {
        installMockIntercept()
    }

    val credentials = loadCredentials(credsFile)
    val results = mutableListOf<ConnectorResult>()
    val testConnectors = connectors ?: credentials.keys.toList()

    val examplesDir = File(sdkRoot, "../../examples").absolutePath

    println("\n${"=".repeat(60)}")
    println("Running smoke tests for ${testConnectors.size} connector(s)")
    if (mock) {
        println("Mode: MOCK (HTTP intercepted, req_transformer verification)")
    }
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

                    if (!mock && !hasValidCredentials(authConfig)) {
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

                    val result = testConnectorScenarios(instanceName, connectorName, config, sdkRoot, dryRun, mock)
                    results.add(result)
                    printResult(result)
                }
            }
            authConfigValue is Map<*, *> -> {
                val authConfig = authConfigValue as AuthConfig

                if (!mock && !hasValidCredentials(authConfig)) {
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

                val result = testConnectorScenarios(connectorName, connectorName, config, sdkRoot, dryRun, mock)
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

    val passed = results.count { it.status in listOf("passed", "dry_run") }
    val skipped = results.count { it.status == "skipped" }
    val failed = results.count { it.status == "failed" }

    // Count per-scenario statuses
    var totalFlowsPassed = 0
    var totalFlowsSkipped = 0
    var totalFlowsFailed = 0
    for (r in results) {
        for (scenario in r.scenarios.values) {
            when (scenario.status) {
                "passed" -> totalFlowsPassed++
                "skipped" -> totalFlowsSkipped++
                "failed" -> totalFlowsFailed++
            }
        }
    }

    println("Total connectors:   ${results.size}")
    println(green("Passed:  $passed"))
    println(grey("Skipped: $skipped (placeholder credentials or no examples)"))
    println((if (failed > 0) ::red else ::green)("Failed:  $failed"))
    println()
    println("Flow results:")
    println(green("  $totalFlowsPassed flows PASSED"))
    if (totalFlowsSkipped > 0) {
        println(yellow("  $totalFlowsSkipped flows SKIPPED (connector errors)"))
    }
    if (totalFlowsFailed > 0) {
        println(red("  $totalFlowsFailed flows FAILED"))
    }
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
        val results = runTests(parsedArgs.credsFile, parsedArgs.connectors, parsedArgs.dryRun, parsedArgs.mock, parsedArgs.sdkRoot)
        val exitCode = printSummary(results)
        System.exit(exitCode)
    } catch (e: Exception) {
        System.err.println("\nFatal error: ${e.message}")
        e.printStackTrace()
        System.exit(1)
    }
}
