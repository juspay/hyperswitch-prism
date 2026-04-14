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
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.LoonioConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("get")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setLoonio(LoonioConfig.newBuilder()
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setMerchantToken(SecretString.newBuilder().setValue("YOUR_MERCHANT_TOKEN").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()



private fun buildGetRequest(connectorTransactionIdStr: String): PaymentServiceGetRequest {
    return PaymentServiceGetRequest.newBuilder().apply {
        merchantTransactionId = "probe_merchant_txn_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    }.build()
}

// Flow: PaymentService.Get
fun get(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildGetRequest("probe_connector_txn_001")
    val response = client.get(request)
    println("Status: ${response.status.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "get"
    when (flow) {
        "get" -> get(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: get")
    }
}
