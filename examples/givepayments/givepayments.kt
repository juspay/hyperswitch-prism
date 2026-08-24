// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py givepayments
//
// Givepayments — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="givepayments processCheckoutCard"

package examples.givepayments

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.EventClient
import payments.RecurringPaymentClient
import payments.RefundClient
import payments.AuthenticationType
import payments.CaptureMethod
import payments.CardNetwork
import payments.Currency
import payments.HttpMethod
import payments.PaymentMethodType
import payments.TokenKind
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.GivepaymentsConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("authorize", "get", "parse_event", "proxy_authorize", "recurring_charge", "refund", "refund_get")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setGivepayments(GivepaymentsConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setBaseUrl("YOUR_BASE_URL")
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
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Authentication Details.
        returnUrl = "https://example.com/return"  // URLs for Redirection and Webhooks.
        browserInfoBuilder.apply {
            userAgent = "Mozilla/5.0 (probe-bot)"
            ipAddress = "1.2.3.4"  // Device Information.
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
        merchantEventId = "probe_event_001"  // Caller-supplied correlation key, echoed in the response. Not used by UCS for processing.
        requestDetailsBuilder.apply {
            method = HttpMethod.HTTP_METHOD_POST  // HTTP method of the request (e.g., GET, POST).
            uri = "https://example.com/webhook"  // URI of the request.
            putAllHeaders(mapOf())  // Headers of the HTTP request.
            body = com.google.protobuf.ByteString.copyFromUtf8("{\"id\": \"GS_EV_pmV0LOyQvHYnG1VNZD2QeE\",\"created_at\": \"1661990400\",\"type\": \"payment.captured\",\"data\": { \"type\": \"payment\", \"object\": { \"id\": \"GS_TXN_cKP1ctmwThYaA5UJrUG67A\", \"id\": \"GS_TXN_cKP1ctmwThYaA5UJrUG67A\", \"created_at\": 1661990400, \"updated_at\": 1661990400, \"settled_at\": null, \"status\": \"successful\", \"processing_state\": \"captured\", \"total_amount\": 500, \"net_amount\": 500, \"fee_amount\": 44, \"fees_paid_by\": \"merchant\", \"description\": \"\", \"reversal_status\": \"not_reversed\", \"billing_descriptor\": \"1OFFICESUPPLIESSTORE\", \"risk\": { \"quarantine\": false, \"risk_level\": \"low\", \"assessment\": \"\" }, \"paymethod\": { \"type\": \"card\", \"card\": { \"id\": \"GS_PMC_7cmafx7A532uIiZaGRsE4D\", \"created_at\": 1661990400, \"updated_at\": 1661990400, \"brand\": \"visa\", \"name\": \"Jack Francis\", \"number_last4\": \"1111\", \"exp_year\": 2023, \"exp_month\": 8, \"is_debit\": false, \"user\": \"GS_USR_5z9QxI1cG1YAAZGV9nos4B\", \"address\": null }}, \"customer\": \"GS_CUS_2rbzrEaeBNwNMafRxKBfSb\", \"merchant\": \"GS_MER_OVC3SKymD34SH5NjEhPa8D\" }}, \"merchant\": \"GS_MER_OVC3SKymD34SH5NjEhPa8D\"}")  // Body of the HTTP request.
        }
    }.build()
    val response = client.handle_event(request)
    println("Webhook: type=${response.eventType.name} verified=${response.sourceVerified}")
}

// Flow: EventService.ParseEvent
fun parseEvent(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = EventClient(config)
    val request = EventServiceParseRequest.newBuilder().apply {
        requestDetailsBuilder.apply {
            method = HttpMethod.HTTP_METHOD_POST  // HTTP method of the request (e.g., GET, POST).
            uri = "https://example.com/webhook"  // URI of the request.
            putAllHeaders(mapOf())  // Headers of the HTTP request.
            body = com.google.protobuf.ByteString.copyFromUtf8("{\"id\": \"GS_EV_pmV0LOyQvHYnG1VNZD2QeE\",\"created_at\": \"1661990400\",\"type\": \"payment.captured\",\"data\": { \"type\": \"payment\", \"object\": { \"id\": \"GS_TXN_cKP1ctmwThYaA5UJrUG67A\", \"id\": \"GS_TXN_cKP1ctmwThYaA5UJrUG67A\", \"created_at\": 1661990400, \"updated_at\": 1661990400, \"settled_at\": null, \"status\": \"successful\", \"processing_state\": \"captured\", \"total_amount\": 500, \"net_amount\": 500, \"fee_amount\": 44, \"fees_paid_by\": \"merchant\", \"description\": \"\", \"reversal_status\": \"not_reversed\", \"billing_descriptor\": \"1OFFICESUPPLIESSTORE\", \"risk\": { \"quarantine\": false, \"risk_level\": \"low\", \"assessment\": \"\" }, \"paymethod\": { \"type\": \"card\", \"card\": { \"id\": \"GS_PMC_7cmafx7A532uIiZaGRsE4D\", \"created_at\": 1661990400, \"updated_at\": 1661990400, \"brand\": \"visa\", \"name\": \"Jack Francis\", \"number_last4\": \"1111\", \"exp_year\": 2023, \"exp_month\": 8, \"is_debit\": false, \"user\": \"GS_USR_5z9QxI1cG1YAAZGV9nos4B\", \"address\": null }}, \"customer\": \"GS_CUS_2rbzrEaeBNwNMafRxKBfSb\", \"merchant\": \"GS_MER_OVC3SKymD34SH5NjEhPa8D\" }}, \"merchant\": \"GS_MER_OVC3SKymD34SH5NjEhPa8D\"}")  // Body of the HTTP request.
        }
    }.build()
    val response = client.parse_event(request)
    println("Webhook parsed: type=${response.eventType.name}")
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
            cardNetwork = CardNetwork.VISA
        }
        customerBuilder.apply {
            emailBuilder.value = "test@example.com"  // Customer's email address.
        }
        addressBuilder.apply {
            billingAddressBuilder.apply {
            }
        }
        captureMethod = CaptureMethod.AUTOMATIC
        authType = AuthenticationType.NO_THREE_DS
        returnUrl = "https://example.com/return"
        browserInfoBuilder.apply {
            userAgent = "Mozilla/5.0 (probe-bot)"
            ipAddress = "1.2.3.4"  // Device Information.
        }
    }.build()
    val response = client.proxy_authorize(request)
    println("Status: ${response.status.name}")
}

// Flow: RecurringPaymentService.Charge
fun recurringCharge(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = RecurringPaymentClient(config)
    val request = RecurringPaymentServiceChargeRequest.newBuilder().apply {
        connectorRecurringPaymentIdBuilder.apply {  // Reference to existing mandate.
            connectorMandateIdBuilder.apply {  // mandate_id sent by the connector.
                connectorMandateIdBuilder.apply {
                    connectorMandateId = "probe-mandate-123"
                }
            }
        }
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        paymentMethodBuilder.apply {  // Optional payment Method Information (for network transaction flows).
            tokenBuilder.apply {  // Payment tokens.
                tokenBuilder.value = "probe_pm_token"  // The token string representing a payment method.
                kind = TokenKind.TOKEN_KIND_MULTI_USE  // Which of the connector's tokenization endpoints minted this token. Connectors that mint more than one kind spend them on different request parameters, so the kind has to travel with the token rather than be inferred from its contents.
            }
        }
        returnUrl = "https://example.com/recurring-return"
        emailBuilder.value = "test@example.com"  // Customer Information.
        connectorCustomerId = "cust_probe_123"
        browserInfoBuilder.apply {  // Browser Information.
            colorDepth = 24  // Display Information.
            screenHeight = 900
            screenWidth = 1440
            javaEnabled = false  // Browser Settings.
            javaScriptEnabled = true
            language = "en-US"
            timeZoneOffsetMinutes = -480
            acceptHeader = "application/json"  // Browser Headers.
            userAgent = "Mozilla/5.0 (probe-bot)"
            acceptLanguage = "en-US,en;q=0.9"
            ipAddress = "1.2.3.4"  // Device Information.
        }
        paymentMethodType = PaymentMethodType.PAY_PAL
        offSession = true  // Behavioral Flags and Preferences.
    }.build()
    val response = client.charge(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Recurring_Charge failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
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
        refundId = "probe_refund_id_001"  // Deprecated.
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
        "get" -> get(txnId)
        "handleEvent" -> handleEvent(txnId)
        "parseEvent" -> parseEvent(txnId)
        "proxyAuthorize" -> proxyAuthorize(txnId)
        "recurringCharge" -> recurringCharge(txnId)
        "refund" -> refund(txnId)
        "refundGet" -> refundGet(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: processCheckoutAutocapture, processRefund, processGetPayment, authorize, get, handleEvent, parseEvent, proxyAuthorize, recurringCharge, refund, refundGet")
    }
}
