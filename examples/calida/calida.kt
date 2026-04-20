// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py calida
//
// Calida — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="calida processCheckoutCard"

package examples.calida

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment


val SUPPORTED_FLOWS = listOf<String>("get", "parse_event")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Calida credentials here
    .build()





fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "authorize"
    when (flow) {

        else -> System.err.println("Unknown flow: $flow. Available: ")
    }
}
