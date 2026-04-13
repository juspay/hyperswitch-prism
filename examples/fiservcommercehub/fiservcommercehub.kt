// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py fiservcommercehub
//
// Fiservcommercehub — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="fiservcommercehub processCheckoutCard"

package examples.fiservcommercehub

import types.Payment.*
import types.PaymentMethods.*
import payments.MerchantAuthenticationClient
import payments.PaymentClient
import payments.RefundClient
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.FiservcommercehubConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("create_server_authentication_token", "get", "refund", "refund_get", "void")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setFiservcommercehub(FiservcommercehubConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setSecret(SecretString.newBuilder().setValue("YOUR_SECRET").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setTerminalId(SecretString.newBuilder().setValue("YOUR_TERMINAL_ID").build())
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
        stateBuilder.apply {  // State Information.
            accessTokenBuilder.apply {  // Access token obtained from connector.
                tokenBuilder.value = "probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch).
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
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
        stateBuilder.apply {  // State data for access token storage and.
            accessTokenBuilder.apply {  // Access token obtained from connector.
                tokenBuilder.value = "probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch).
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    }.build()
}

private fun buildVoidRequest(connectorTransactionIdStr: String): PaymentServiceVoidRequest {
    return PaymentServiceVoidRequest.newBuilder().apply {
        merchantVoidId = "probe_void_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        stateBuilder.apply {  // State Information.
            accessTokenBuilder.apply {  // Access token obtained from connector.
                tokenBuilder.value = "probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch).
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    }.build()
}

// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
fun createServerAuthenticationToken(txnId: String) {
    val client = MerchantAuthenticationClient(_defaultConfig)
    val request = MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest.newBuilder().apply {

    }.build()
    val response = client.create_server_authentication_token(request)
    println("StatusCode: ${response.statusCode}")
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

// Flow: RefundService.Get
fun refundGet(txnId: String) {
    val client = RefundClient(_defaultConfig)
    val request = RefundServiceGetRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"  // Identification.
        connectorTransactionId = "probe_connector_txn_001"
        refundId = "probe_refund_id_001"
        stateBuilder.apply {  // State Information.
            accessTokenBuilder.apply {  // Access token obtained from connector.
                tokenBuilder.value = "probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch).
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    }.build()
    val response = client.refund_get(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.Void
fun void(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildVoidRequest("probe_connector_txn_001")
    val response = client.void(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Void failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "createServerAuthenticationToken"
    when (flow) {
        "createServerAuthenticationToken" -> createServerAuthenticationToken(txnId)
        "get" -> get(txnId)
        "refund" -> refund(txnId)
        "refundGet" -> refundGet(txnId)
        "void" -> void(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: createServerAuthenticationToken, get, refund, refundGet, void")
    }
}
