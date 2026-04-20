// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py loonio
//
// Loonio — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="loonio processCheckoutCard"

package examples.loonio

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment


val SUPPORTED_FLOWS = listOf<String>("get")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Loonio credentials here
    .build()





fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "authorize"
    when (flow) {

        else -> System.err.println("Unknown flow: $flow. Available: ")
    }
}
