// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py kount
//
// Kount — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="kount processCheckoutCard"

package examples.kount

import types.Payment.*
import types.PaymentMethods.*
import payments.MerchantAuthenticationClient
import payments.PaymentMethodAuthenticationClient
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.KountConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("create_server_authentication_token", "pre_authenticate")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setKount(KountConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setAuthServerId("YOUR_AUTH_SERVER_ID")
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

// Flow: PaymentMethodAuthenticationService.PreAuthenticate
fun preAuthenticate(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentMethodAuthenticationClient(config)
    val request = PaymentMethodAuthenticationServicePreAuthenticateRequest.newBuilder().apply {

    }.build()
    val response = client.pre_authenticate(request)
    println("Status: ${response.status.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "createServerAuthenticationToken"
    when (flow) {
        "createServerAuthenticationToken" -> createServerAuthenticationToken(txnId)
        "preAuthenticate" -> preAuthenticate(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: createServerAuthenticationToken, preAuthenticate")
    }
}
