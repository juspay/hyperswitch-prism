// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py itaubank
//
// Itaubank — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="itaubank processCheckoutCard"

package examples.itaubank

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment


val SUPPORTED_FLOWS = listOf<String>("create_server_authentication_token")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Itaubank credentials here
    .build()





fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "authorize"
    when (flow) {

        else -> System.err.println("Unknown flow: $flow. Available: ")
    }
}
