// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py gigadat
//
// Gigadat — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="gigadat processCheckoutCard"

package examples.gigadat

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.GigadatConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("get", "refund")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setGigadat(GigadatConfig.newBuilder()
                .setCampaignId(SecretString.newBuilder().setValue("YOUR_CAMPAIGN_ID").build())
                .setAccessToken(SecretString.newBuilder().setValue("YOUR_ACCESS_TOKEN").build())
                .setSecurityToken(SecretString.newBuilder().setValue("YOUR_SECURITY_TOKEN").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setSite("YOUR_SITE")
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

private fun buildRefundRequest(connectorTransactionIdStr: String): PaymentServiceRefundRequest {
    return PaymentServiceRefundRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        paymentAmount = 1000L  // Amount Information.
        refundAmountBuilder.apply {
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        reason = "customer_request"  // Reason for the refund.
    }.build()
}

// Flow: PaymentService.Get
fun get(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildGetRequest("probe_connector_txn_001")
    val response = client.get(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.Refund
fun refund(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildRefundRequest("probe_connector_txn_001")
    val response = client.refund(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Refund failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "get"
    when (flow) {
        "get" -> get(txnId)
        "refund" -> refund(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: get, refund")
    }
}
