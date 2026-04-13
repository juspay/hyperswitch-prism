// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py iatapay
//
// Iatapay — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="iatapay processCheckoutCard"

package examples.iatapay

import payments.PaymentClient
import payments.MerchantAuthenticationClient
import payments.PaymentServiceAuthorizeRequest
import payments.MerchantAuthenticationServiceCreateAccessTokenRequest
import payments.PaymentServiceGetRequest
import payments.PaymentServiceRefundRequest
import payments.AuthenticationType
import payments.CaptureMethod
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment


private fun buildAuthorizeRequest(captureMethodStr: String): PaymentServiceAuthorizeRequest {
    return PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"  // Identification
        amountBuilder.apply {  // The amount for the payment
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00)
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR")
        }
        paymentMethodBuilder.apply {  // Payment method to be used
            idealBuilder.apply {
            }
        }
        captureMethod = CaptureMethod.valueOf(captureMethodStr)  // Method for capturing the payment
        addressBuilder.apply {  // Address Information
            billingAddressBuilder.apply {
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Authentication Details
        returnUrl = "https://example.com/return"  // URLs for Redirection and Webhooks
        webhookUrl = "https://example.com/webhook"
        stateBuilder.apply {  // State Information
            accessTokenBuilder.apply {  // Access token obtained from connector
                token = "probe_access_token"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch)
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    }.build()
}

private fun buildGetRequest(connectorTransactionIdStr: String): PaymentServiceGetRequest {
    return PaymentServiceGetRequest.newBuilder().apply {
        merchantTransactionId = "probe_merchant_txn_001"  // Identification
        connectorTransactionId = connectorTransactionIdStr
        amountBuilder.apply {  // Amount Information
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00)
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR")
        }
        stateBuilder.apply {  // State Information
            accessTokenBuilder.apply {  // Access token obtained from connector
                token = "probe_access_token"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch)
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
        connectorOrderReferenceId = "probe_order_ref_001"  // Connector Reference Id
    }.build()
}

private fun buildRefundRequest(connectorTransactionIdStr: String): PaymentServiceRefundRequest {
    return PaymentServiceRefundRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"  // Identification
        connectorTransactionId = connectorTransactionIdStr
        paymentAmount = 1000L  // Amount Information
        refundAmountBuilder.apply {
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00)
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR")
        }
        reason = "customer_request"  // Reason for the refund
        webhookUrl = "https://example.com/webhook"  // URL for webhook notifications
        stateBuilder.apply {  // State data for access token storage and other connector-specific state
            accessTokenBuilder.apply {  // Access token obtained from connector
                token = "probe_access_token"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch)
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    }.build()
}

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your connector config here
    .build()


// Flow: PaymentService.Authorize (Ideal)
fun authorize(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildAuthorizeRequest("AUTOMATIC")
    val response = client.authorize(request)
    when (response.status.name) {
        "FAILED"  -> throw RuntimeException("Authorize failed: ${response.error.unifiedDetails.message}")
        "PENDING" -> println("Pending — await webhook before proceeding")
        else      -> println("Authorized: ${response.connectorTransactionId}")
    }
}

// Flow: MerchantAuthenticationService.CreateAccessToken
fun createAccessToken(txnId: String) {
    val client = MerchantAuthenticationClient(_defaultConfig)
    val request = MerchantAuthenticationServiceCreateAccessTokenRequest.newBuilder().apply {

    }.build()
    val response = client.create_access_token(request)
    println("Access token obtained (statusCode=${response.statusCode})")
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
    val flow = args.firstOrNull() ?: "authorize"
    when (flow) {
        "authorize" -> authorize(txnId)
        "createAccessToken" -> createAccessToken(txnId)
        "get" -> get(txnId)
        "refund" -> refund(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: authorize, createAccessToken, get, refund")
    }
}
