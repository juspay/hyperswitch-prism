// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py stax
//
// Stax — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="stax processCheckoutCard"

package examples.stax

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment


val SUPPORTED_FLOWS = listOf<String>("capture", "create_customer", "get", "refund", "refund_get", "token_authorize", "tokenize", "void")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Stax credentials here
    .build()





fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "authorize"
    when (flow) {

        else -> System.err.println("Unknown flow: $flow. Available: ")
    }
}
