// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py itaubank
//
// Itaubank — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="itaubank processCheckoutCard"

package examples.itaubank

import types.Payment.*
import types.PaymentMethods.*
import payments.MerchantAuthenticationClient
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.ItaubankConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("create_server_authentication_token")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setItaubank(ItaubankConfig.newBuilder()
                .setClientSecret(SecretString.newBuilder().setValue("YOUR_CLIENT_SECRET").build())
                .setClientId(SecretString.newBuilder().setValue("YOUR_CLIENT_ID").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()


// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
fun createServerAuthenticationToken(txnId: String) {
    val client = MerchantAuthenticationClient(_defaultConfig)
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
