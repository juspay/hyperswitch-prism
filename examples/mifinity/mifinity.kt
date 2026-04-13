// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py mifinity
//
// Mifinity — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="mifinity processCheckoutCard"

package examples.mifinity

import payments.MerchantAuthenticationClient
import payments.PaymentClient
import payments.MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest
import payments.PaymentServiceGetRequest
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment


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

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your connector config here
    .build()


// Flow: MerchantAuthenticationService.CreateClientAuthenticationToken
fun createClientAuthenticationToken(txnId: String) {
    val client = MerchantAuthenticationClient(_defaultConfig)
    val request = MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest.newBuilder().apply {
        merchantClientSessionId = "probe_sdk_session_001"  // Infrastructure.
        paymentBuilder.apply {  // FrmClientAuthenticationContext frm = 5; // future: device fingerprinting PayoutClientAuthenticationContext payout = 6; // future: payout verification widget.
            amountBuilder.apply {
                minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
                currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
            }
        }
    }.build()
    val response = client.create_client_authentication_token(request)
    println("StatusCode: ${response.statusCode}")
}

// Flow: PaymentService.Get
fun get(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildGetRequest("probe_connector_txn_001")
    val response = client.get(request)
    println("Status: ${response.status.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "createClientAuthenticationToken"
    when (flow) {
        "createClientAuthenticationToken" -> createClientAuthenticationToken(txnId)
        "get" -> get(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: createClientAuthenticationToken, get")
    }
}
