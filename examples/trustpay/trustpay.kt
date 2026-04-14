// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py trustpay
//
// Trustpay — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="trustpay processCheckoutCard"

package examples.trustpay

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.MerchantAuthenticationClient
import payments.EventClient
import payments.RefundClient
import payments.AuthenticationType
import payments.CaptureMethod
import payments.CountryAlpha2
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.TrustpayConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("authorize", "create_order", "create_server_authentication_token", "get", "proxy_authorize", "refund", "refund_get")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setTrustpay(TrustpayConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setProjectId(SecretString.newBuilder().setValue("YOUR_PROJECT_ID").build())
                .setSecretKey(SecretString.newBuilder().setValue("YOUR_SECRET_KEY").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setBaseUrlBankRedirects("YOUR_BASE_URL_BANK_REDIRECTS")
                .build())
            .build()
    )
    .build()



private fun buildAuthorizeRequest(captureMethodStr: String): PaymentServiceAuthorizeRequest {
    return PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"  // Identification.
        amountBuilder.apply {  // The amount for the payment.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        paymentMethodBuilder.apply {  // Payment method to be used.
            cardBuilder.apply {  // Generic card payment.
                cardNumberBuilder.value = "4111111111111111"  // Card Identification.
                cardExpMonthBuilder.value = "03"
                cardExpYearBuilder.value = "2030"
                cardCvcBuilder.value = "737"
                cardHolderNameBuilder.value = "John Doe"  // Cardholder Information.
            }
        }
        captureMethod = CaptureMethod.valueOf(captureMethodStr)  // Method for capturing the payment.
        customerBuilder.apply {  // Customer Information.
            emailBuilder.value = "test@example.com"  // Customer's email address.
        }
        addressBuilder.apply {  // Address Information.
            billingAddressBuilder.apply {
                firstNameBuilder.value = "John"  // Personal Information.
                line1Builder.value = "123 Main St"  // Address Details.
                cityBuilder.value = "Seattle"
                zipCodeBuilder.value = "98101"
                countryAlpha2Code = CountryAlpha2.US
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Authentication Details.
        returnUrl = "https://example.com/return"  // URLs for Redirection and Webhooks.
        browserInfoBuilder.apply {
            userAgent = "Mozilla/5.0 (probe-bot)"
            ipAddress = "1.2.3.4"  // Device Information.
        }
        stateBuilder.apply {  // State Information.
            accessTokenBuilder.apply {  // Access token obtained from connector.
                tokenBuilder.value = "probe_access_token"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch).
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    }.build()
}

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
                tokenBuilder.value = "probe_access_token"  // The token string.
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
                tokenBuilder.value = "probe_access_token"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch).
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    }.build()
}

// Scenario: One-step Payment (Authorize + Capture)
// Simple payment that authorizes and captures in one call. Use for immediate charges.
fun processCheckoutAutocapture(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val authorizeResponse = paymentClient.authorize(buildAuthorizeRequest("AUTOMATIC"))

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    return mapOf("status" to authorizeResponse.status.name, "transactionId" to authorizeResponse.connectorTransactionId, "error" to authorizeResponse.error)
}

// Scenario: Refund
// Return funds to the customer for a completed payment.
fun processRefund(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val authorizeResponse = paymentClient.authorize(buildAuthorizeRequest("AUTOMATIC"))

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    // Step 2: Refund — return funds to the customer
    val refundResponse = paymentClient.refund(buildRefundRequest(authorizeResponse.connectorTransactionId ?: ""))

    if (refundResponse.status.name == "FAILED")
        throw RuntimeException("Refund failed: ${refundResponse.error.unifiedDetails.message}")

    return mapOf("status" to refundResponse.status.name, "error" to refundResponse.error)
}

// Scenario: Get Payment Status
// Retrieve current payment status from the connector.
fun processGetPayment(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val authorizeResponse = paymentClient.authorize(buildAuthorizeRequest("MANUAL"))

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    // Step 2: Get — retrieve current payment status from the connector
    val getResponse = paymentClient.get(buildGetRequest(authorizeResponse.connectorTransactionId ?: ""))

    return mapOf("status" to getResponse.status.name, "transactionId" to getResponse.connectorTransactionId, "error" to getResponse.error)
}

// Flow: PaymentService.Authorize (Card)
fun authorize(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildAuthorizeRequest("AUTOMATIC")
    val response = client.authorize(request)
    when (response.status.name) {
        "FAILED"  -> throw RuntimeException("Authorize failed: ${response.error.unifiedDetails.message}")
        "PENDING" -> println("Pending — await webhook before proceeding")
        else      -> println("Authorized: ${response.connectorTransactionId}")
    }
}

// Flow: PaymentService.CreateOrder
fun createOrder(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceCreateOrderRequest.newBuilder().apply {
        merchantOrderId = "probe_order_001"  // Identification.
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        stateBuilder.apply {  // State Information.
            accessTokenBuilder.apply {  // Access token obtained from connector.
                tokenBuilder.value = "probe_access_token"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch).
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    }.build()
    val response = client.create_order(request)
    println("Order: ${response.connectorOrderId}")
}

// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
fun createServerAuthenticationToken(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = MerchantAuthenticationClient(config)
    val request = MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest.newBuilder().apply {

    }.build()
    val response = client.create_server_authentication_token(request)
    println("StatusCode: ${response.statusCode}")
}

// Flow: PaymentService.Get
fun get(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildGetRequest("probe_connector_txn_001")
    val response = client.get(request)
    println("Status: ${response.status.name}")
}

// Flow: EventService.HandleEvent
fun handleEvent(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = EventClient(config)
    val request = EventServiceHandleRequest.newBuilder().apply {

    }.build()
    val response = client.handle_event(request)
    println("Event status: ${response.eventStatus.name}")
}

// Flow: PaymentService.ProxyAuthorize
fun proxyAuthorize(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceProxyAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_proxy_txn_001"
        amountBuilder.apply {
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        cardProxyBuilder.apply {  // Card proxy for vault-aliased payments (VGS, Basis Theory, Spreedly). Real card values are substituted by the proxy before reaching the connector.
            cardNumberBuilder.value = "4111111111111111"  // Card Identification.
            cardExpMonthBuilder.value = "03"
            cardExpYearBuilder.value = "2030"
            cardCvcBuilder.value = "123"
            cardHolderNameBuilder.value = "John Doe"  // Cardholder Information.
        }
        customerBuilder.apply {
            emailBuilder.value = "test@example.com"  // Customer's email address.
        }
        addressBuilder.apply {
            billingAddressBuilder.apply {
                firstNameBuilder.value = "John"  // Personal Information.
                line1Builder.value = "123 Main St"  // Address Details.
                cityBuilder.value = "Seattle"
                zipCodeBuilder.value = "98101"
                countryAlpha2Code = CountryAlpha2.US
            }
        }
        captureMethod = CaptureMethod.AUTOMATIC
        authType = AuthenticationType.NO_THREE_DS
        returnUrl = "https://example.com/return"
        browserInfoBuilder.apply {
            userAgent = "Mozilla/5.0 (probe-bot)"
            ipAddress = "1.2.3.4"  // Device Information.
        }
        stateBuilder.apply {
            accessTokenBuilder.apply {  // Access token obtained from connector.
                tokenBuilder.value = "probe_access_token"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch).
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    }.build()
    val response = client.proxy_authorize(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.Refund
fun refund(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildRefundRequest("probe_connector_txn_001")
    val response = client.refund(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Refund failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}

// Flow: RefundService.Get
fun refundGet(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = RefundClient(config)
    val request = RefundServiceGetRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"  // Identification.
        connectorTransactionId = "probe_connector_txn_001"
        refundId = "probe_refund_id_001"
        stateBuilder.apply {  // State Information.
            accessTokenBuilder.apply {  // Access token obtained from connector.
                tokenBuilder.value = "probe_access_token"  // The token string.
                expiresInSeconds = 3600L  // Expiration timestamp (seconds since epoch).
                tokenType = "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    }.build()
    val response = client.refund_get(request)
    println("Status: ${response.status.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "processCheckoutAutocapture"
    when (flow) {
        "processCheckoutAutocapture" -> processCheckoutAutocapture(txnId)
        "processRefund" -> processRefund(txnId)
        "processGetPayment" -> processGetPayment(txnId)
        "authorize" -> authorize(txnId)
        "createOrder" -> createOrder(txnId)
        "createServerAuthenticationToken" -> createServerAuthenticationToken(txnId)
        "get" -> get(txnId)
        "handleEvent" -> handleEvent(txnId)
        "proxyAuthorize" -> proxyAuthorize(txnId)
        "refund" -> refund(txnId)
        "refundGet" -> refundGet(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: processCheckoutAutocapture, processRefund, processGetPayment, authorize, createOrder, createServerAuthenticationToken, get, handleEvent, proxyAuthorize, refund, refundGet")
    }
}
