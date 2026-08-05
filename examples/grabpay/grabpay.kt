// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py grabpay
//
// Grabpay — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="grabpay processCheckoutCard"

package examples.grabpay

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.GrabpayConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("create_order")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setGrabpay(GrabpayConfig.newBuilder()
                .setPartnerId(SecretString.newBuilder().setValue("YOUR_PARTNER_ID").build())
                .setPartnerSecret(SecretString.newBuilder().setValue("YOUR_PARTNER_SECRET").build())
                .setClientId(SecretString.newBuilder().setValue("YOUR_CLIENT_ID").build())
                .setClientSecret(SecretString.newBuilder().setValue("YOUR_CLIENT_SECRET").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()


// Flow: PaymentService.CreateOrder
fun createOrder(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceCreateOrderRequest.newBuilder().apply {
        merchantOrderId = "probe_order_001"  // Identification.
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    }.build()
    val response = client.create_order(request)
    println("Order: ${response.connectorOrderId}")
}

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
    val flow = args.firstOrNull() ?: "createOrder"
    when (flow) {
        "createOrder" -> createOrder(txnId)
        "verifyRedirect" -> verifyRedirect(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: createOrder, verifyRedirect")
    }
}
