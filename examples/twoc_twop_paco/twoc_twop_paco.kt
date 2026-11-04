// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py twoc_twop_paco
//
// Twoc_Twop_Paco — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="twoc_twop_paco processCheckoutCard"

package examples.twoc_twop_paco

import types.Payment.*
import types.Events.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.TwocTwopPacoConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>()

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setTwocTwopPaco(TwocTwopPacoConfig.newBuilder()
                .setAccessToken(SecretString.newBuilder().setValue("YOUR_ACCESS_TOKEN").build())
                .setOfficeId(SecretString.newBuilder().setValue("YOUR_OFFICE_ID").build())
                .setPacoKid(SecretString.newBuilder().setValue("YOUR_PACO_KID").build())
                .setMerchantSigningPrivateKey(SecretString.newBuilder().setValue("YOUR_MERCHANT_SIGNING_PRIVATE_KEY").build())
                .setMerchantEncryptionPrivateKey(SecretString.newBuilder().setValue("YOUR_MERCHANT_ENCRYPTION_PRIVATE_KEY").build())
                .setPacoSigningPublicKey(SecretString.newBuilder().setValue("YOUR_PACO_SIGNING_PUBLIC_KEY").build())
                .setPacoEncryptionPublicKey(SecretString.newBuilder().setValue("YOUR_PACO_ENCRYPTION_PUBLIC_KEY").build())
                .setResponseAudience(SecretString.newBuilder().setValue("YOUR_RESPONSE_AUDIENCE").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()


// Flow: PaymentService.VerifyRedirectResponse
fun verifyRedirect(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceVerifyRedirectResponseRequest.newBuilder().apply {

    }.build()
    val response = client.verify_redirect_response(request)
    println("Source verified: ${response.sourceVerified}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "verifyRedirect"
    when (flow) {
        "verifyRedirect" -> verifyRedirect(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: verifyRedirect")
    }
}
