// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py trustly
//
// Trustly — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="trustly processCheckoutCard"

package examples.trustly

import types.Payment.*
import types.PaymentMethods.*
import payments.EventClient
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.TrustlyConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>()

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setTrustly(TrustlyConfig.newBuilder()
                .setUsername(SecretString.newBuilder().setValue("YOUR_USERNAME").build())
                .setPassword(SecretString.newBuilder().setValue("YOUR_PASSWORD").build())
                .setPrivateKey(SecretString.newBuilder().setValue("YOUR_PRIVATE_KEY").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()


// Flow: EventService.HandleEvent
fun handleEvent(txnId: String) {
    val client = EventClient(_defaultConfig)
    val request = EventServiceHandleRequest.newBuilder().apply {

    }.build()
    val response = client.handle_event(request)
    println("Event status: ${response.eventStatus.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "handleEvent"
    when (flow) {
        "handleEvent" -> handleEvent(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: handleEvent")
    }
}
