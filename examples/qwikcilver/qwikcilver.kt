// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py qwikcilver
//
// Qwikcilver — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="qwikcilver processCheckoutCard"

package examples.qwikcilver

import types.Payment.*
import types.PaymentMethods.*
import payments.MerchantAuthenticationClient
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.QwikcilverConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("create_server_authentication_token")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setQwikcilver(QwikcilverConfig.newBuilder()
                .setBootstrapBearerToken(SecretString.newBuilder().setValue("YOUR_BOOTSTRAP_BEARER_TOKEN").build())
                .setTerminalId(SecretString.newBuilder().setValue("YOUR_TERMINAL_ID").build())
                .setUsername(SecretString.newBuilder().setValue("YOUR_USERNAME").build())
                .setPassword(SecretString.newBuilder().setValue("YOUR_PASSWORD").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()


// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
fun createServerAuthenticationToken(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = MerchantAuthenticationClient(config)
    val request = MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest.newBuilder().apply {

    }.build()
    val response = client.create_server_authentication_token(request)
    println("StatusCode: ${response.statusCode}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "createServerAuthenticationToken"
    when (flow) {
        "createServerAuthenticationToken" -> createServerAuthenticationToken(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: createServerAuthenticationToken")
    }
}
