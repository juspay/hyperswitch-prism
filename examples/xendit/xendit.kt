// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py xendit
//
// Xendit — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="xendit processCheckoutCard"

package examples.xendit

import types.Payment.*
import types.Events.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.EventClient
import payments.RecurringPaymentClient
import payments.RefundClient
import payments.AcceptanceType
import payments.AuthenticationType
import payments.CaptureMethod
import payments.CardNetwork
import payments.Currency
import payments.FutureUsage
import payments.HttpMethod
import payments.PaymentMethodType
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.XenditConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("authorize", "get", "parse_event", "proxy_authorize", "proxy_setup_recurring", "recurring_charge", "refund", "refund_get", "setup_recurring")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setXendit(XenditConfig.newBuilder()
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
        addressBuilder.apply {  // Address Information.
            billingAddressBuilder.apply {
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Authentication Details.
        returnUrl = "https://example.com/return"  // URLs for Redirection and Webhooks.
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
        merchantEventId = "probe_event_001"
        requestDetailsBuilder.apply {
            method = HttpMethod.HTTP_METHOD_POST  // HTTP method of the request (e.g., GET, POST).
            uri = "https://example.com/webhook"  // URI of the request.
            putAllHeaders(mapOf())  // Headers of the HTTP request.
            body = com.google.protobuf.ByteString.copyFromUtf8("{\"created\":\"2024-12-18T05:46:35.109Z\",\"business_id\":\"62440e322008e87fb29c1fd0\",\"event\":\"payment.authorization\",\"data\":{\"type\":\"PAY\",\"status\":\"AUTHORIZED\",\"country\":\"ID\",\"created\":\"2024-12-18T05:46:08.192Z\",\"updated\":\"2024-12-18T05:46:30.627Z\",\"currency\":\"IDR\",\"payment_id\":\"py-3f57d678-2448-4c9f-a433-8468d366fb5c\",\"business_id\":\"62440e322008e87fb29c1fd0\",\"customer_id\":\"cust-7de9a9b4-37e8-40ad-b665-d97f42e538c5\",\"channel_code\":\"CARDS\",\"reference_id\":\"97ba0a32-b996-4abf-8a7b-6184a6644676_b8d18f2f-3\",\"capture_method\":\"MANUAL\",\"request_amount\":10000,\"payment_details\":{\"authorization_data\":{\"reconciliation_id\":\"7345007929096981703954\",\"authorization_code\":\"831000\",\"acquirer_merchant_id\":\"xendit_ctv_agg\",\"network_response_code\":\"00\",\"network_transaction_id\":\"016153570198200\",\"cvn_verification_result\":\"M\",\"retrieval_reference_number\":\"435205253972\",\"address_verification_result\":\"M\",\"network_response_code_descriptor\":\"Approved and completed sucessfully\"},\"authentication_data\":{\"flow\":\"CHALLENGE\",\"a_res\":{\"eci\":\"05\",\"message_version\":\"2.1.0\",\"authentication_value\":\"AAIBBYNoEwAAACcKhAJkdQAAAAA=\",\"directory_server_trans_id\":\"e537f539-d59f-4ebe-8d56-7fdc31a8e9b4\"}}},\"payment_request_id\":\"pr-5593127f-8c7b-4d2f-b487-c785ffc21e2f\"},\"api_version\":\"v3\"}")  // Body of the HTTP request.
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
            body = com.google.protobuf.ByteString.copyFromUtf8("{\"created\":\"2024-12-18T05:46:35.109Z\",\"business_id\":\"62440e322008e87fb29c1fd0\",\"event\":\"payment.authorization\",\"data\":{\"type\":\"PAY\",\"status\":\"AUTHORIZED\",\"country\":\"ID\",\"created\":\"2024-12-18T05:46:08.192Z\",\"updated\":\"2024-12-18T05:46:30.627Z\",\"currency\":\"IDR\",\"payment_id\":\"py-3f57d678-2448-4c9f-a433-8468d366fb5c\",\"business_id\":\"62440e322008e87fb29c1fd0\",\"customer_id\":\"cust-7de9a9b4-37e8-40ad-b665-d97f42e538c5\",\"channel_code\":\"CARDS\",\"reference_id\":\"97ba0a32-b996-4abf-8a7b-6184a6644676_b8d18f2f-3\",\"capture_method\":\"MANUAL\",\"request_amount\":10000,\"payment_details\":{\"authorization_data\":{\"reconciliation_id\":\"7345007929096981703954\",\"authorization_code\":\"831000\",\"acquirer_merchant_id\":\"xendit_ctv_agg\",\"network_response_code\":\"00\",\"network_transaction_id\":\"016153570198200\",\"cvn_verification_result\":\"M\",\"retrieval_reference_number\":\"435205253972\",\"address_verification_result\":\"M\",\"network_response_code_descriptor\":\"Approved and completed sucessfully\"},\"authentication_data\":{\"flow\":\"CHALLENGE\",\"a_res\":{\"eci\":\"05\",\"message_version\":\"2.1.0\",\"authentication_value\":\"AAIBBYNoEwAAACcKhAJkdQAAAAA=\",\"directory_server_trans_id\":\"e537f539-d59f-4ebe-8d56-7fdc31a8e9b4\"}}},\"payment_request_id\":\"pr-5593127f-8c7b-4d2f-b487-c785ffc21e2f\"},\"api_version\":\"v3\"}")  // Body of the HTTP request.
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
        addressBuilder.apply {
            billingAddressBuilder.apply {
            }
        }
        captureMethod = CaptureMethod.AUTOMATIC
        authType = AuthenticationType.NO_THREE_DS
        returnUrl = "https://example.com/return"
    }.build()
    val response = client.proxy_authorize(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.ProxySetupRecurring
fun proxySetupRecurring(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceProxySetupRecurringRequest.newBuilder().apply {
        merchantRecurringPaymentId = "probe_proxy_mandate_001"
        amountBuilder.apply {
            minorAmount = 0L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        cardProxyBuilder.apply {  // Card proxy for vault-aliased payments.
            cardNumberBuilder.value = "4111111111111111"  // Card Identification.
            cardExpMonthBuilder.value = "03"
            cardExpYearBuilder.value = "2030"
            cardCvcBuilder.value = "123"
            cardHolderNameBuilder.value = "John Doe"  // Cardholder Information.
            cardNetwork = CardNetwork.VISA
        }
        addressBuilder.apply {
            billingAddressBuilder.apply {
            }
        }
        customerAcceptanceBuilder.apply {
            acceptanceType = AcceptanceType.OFFLINE  // Type of acceptance (e.g., online, offline).
            acceptedAt = 0L  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        }
        authType = AuthenticationType.NO_THREE_DS
        setupFutureUsage = FutureUsage.OFF_SESSION
    }.build()
    val response = client.proxy_setup_recurring(request)
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
            }
        }
        returnUrl = "https://example.com/recurring-return"
        connectorCustomerId = "cust_probe_123"
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

// Flow: PaymentService.SetupRecurring
fun setupRecurring(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceSetupRecurringRequest.newBuilder().apply {
        merchantRecurringPaymentId = "probe_mandate_001"  // Identification.
        amountBuilder.apply {  // Mandate Details.
            minorAmount = 0L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        paymentMethodBuilder.apply {
            cardBuilder.apply {  // Generic card payment.
                cardNumberBuilder.value = "4111111111111111"  // Card Identification.
                cardExpMonthBuilder.value = "03"
                cardExpYearBuilder.value = "2030"
                cardCvcBuilder.value = "737"
                cardHolderNameBuilder.value = "John Doe"  // Cardholder Information.
            }
        }
        addressBuilder.apply {  // Address Information.
            billingAddressBuilder.apply {
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Type of authentication to be used.
        enrolledFor3Ds = false  // Indicates if the customer is enrolled for 3D Secure.
        returnUrl = "https://example.com/mandate-return"  // URL to redirect after setup.
        setupFutureUsage = FutureUsage.OFF_SESSION  // Indicates future usage intention.
        requestIncrementalAuthorization = false  // Indicates if incremental authorization is requested.
        customerAcceptanceBuilder.apply {  // Details of customer acceptance.
            acceptanceType = AcceptanceType.OFFLINE  // Type of acceptance (e.g., online, offline).
            acceptedAt = 0L  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        }
    }.build()
    val response = client.setup_recurring(request)
    when (response.status.name) {
        "FAILED" -> throw RuntimeException("Setup failed: ${response.error.unifiedDetails.message}")
        else     -> println("Mandate stored: ${response.connectorRecurringPaymentId}")
    }
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
        "proxySetupRecurring" -> proxySetupRecurring(txnId)
        "recurringCharge" -> recurringCharge(txnId)
        "refund" -> refund(txnId)
        "refundGet" -> refundGet(txnId)
        "setupRecurring" -> setupRecurring(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: processCheckoutAutocapture, processRefund, processGetPayment, authorize, get, handleEvent, parseEvent, proxyAuthorize, proxySetupRecurring, recurringCharge, refund, refundGet, setupRecurring")
    }
}
