// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py adyen
//
// Adyen — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="adyen processCheckoutCard"

package examples.adyen

import types.Payment.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.MerchantAuthenticationClient
import payments.DisputeClient
import payments.EventClient
import payments.RecurringPaymentClient
import payments.AcceptanceType
import payments.AuthenticationType
import payments.CaptureMethod
import payments.Currency
import payments.FutureUsage
import payments.HttpMethod
import payments.PaymentMethodType
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.AdyenConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("authorize", "capture", "create_client_authentication_token", "create_order", "dispute_accept", "dispute_defend", "dispute_submit_evidence", "incremental_authorization", "parse_event", "proxy_authorize", "proxy_setup_recurring", "recurring_charge", "refund", "setup_recurring", "token_authorize", "void")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setAdyen(AdyenConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setMerchantAccount(SecretString.newBuilder().setValue("YOUR_MERCHANT_ACCOUNT").build())
                .setReviewKey(SecretString.newBuilder().setValue("YOUR_REVIEW_KEY").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setDisputeBaseUrl("YOUR_DISPUTE_BASE_URL")
                .setEndpointPrefix("YOUR_ENDPOINT_PREFIX")
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
        browserInfoBuilder.apply {
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
    }.build()
}

private fun buildCaptureRequest(connectorTransactionIdStr: String): PaymentServiceCaptureRequest {
    return PaymentServiceCaptureRequest.newBuilder().apply {
        merchantCaptureId = "probe_capture_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        amountToCaptureBuilder.apply {  // Capture Details.
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

private fun buildVoidRequest(connectorTransactionIdStr: String): PaymentServiceVoidRequest {
    return PaymentServiceVoidRequest.newBuilder().apply {
        merchantVoidId = "probe_void_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
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

// Scenario: Card Payment (Authorize + Capture)
// Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
fun processCheckoutCard(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val authorizeResponse = paymentClient.authorize(buildAuthorizeRequest("MANUAL"))

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    // Step 2: Capture — settle the reserved funds
    val captureResponse = paymentClient.capture(buildCaptureRequest(authorizeResponse.connectorTransactionId ?: ""))

    if (captureResponse.status.name == "FAILED")
        throw RuntimeException("Capture failed: ${captureResponse.error.unifiedDetails.message}")

    return mapOf("status" to captureResponse.status.name, "transactionId" to authorizeResponse.connectorTransactionId, "error" to authorizeResponse.error)
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

// Scenario: Void Payment
// Cancel an authorized but not-yet-captured payment.
fun processVoidPayment(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val authorizeResponse = paymentClient.authorize(buildAuthorizeRequest("MANUAL"))

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    // Step 2: Void — release reserved funds (cancel authorization)
    val voidResponse = paymentClient.void(buildVoidRequest(authorizeResponse.connectorTransactionId ?: ""))

    return mapOf("status" to voidResponse.status.name, "transactionId" to authorizeResponse.connectorTransactionId, "error" to voidResponse.error)
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

// Flow: PaymentService.Capture
fun capture(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildCaptureRequest("probe_connector_txn_001")
    val response = client.capture(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Capture failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}

// Flow: MerchantAuthenticationService.CreateClientAuthenticationToken
fun createClientAuthenticationToken(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = MerchantAuthenticationClient(config)
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

// Flow: DisputeService.Accept
fun disputeAccept(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = DisputeClient(config)
    val request = DisputeServiceAcceptRequest.newBuilder().apply {
        merchantDisputeId = "probe_dispute_001"  // Identification.
        connectorTransactionId = "probe_txn_001"
        disputeId = "probe_dispute_id_001"
    }.build()
    val response = client.accept(request)
    println("Dispute status: ${response.disputeStatus.name}")
}

// Flow: DisputeService.Defend
fun disputeDefend(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = DisputeClient(config)
    val request = DisputeServiceDefendRequest.newBuilder().apply {
        merchantDisputeId = "probe_dispute_001"  // Identification.
        connectorTransactionId = "probe_txn_001"
        disputeId = "probe_dispute_id_001"
        reasonCode = "probe_reason"  // Defend Details.
    }.build()
    val response = client.defend(request)
    println("Dispute status: ${response.disputeStatus.name}")
}

// Flow: DisputeService.SubmitEvidence
fun disputeSubmitEvidence(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = DisputeClient(config)
    val request = DisputeServiceSubmitEvidenceRequest.newBuilder().apply {
        merchantDisputeId = "probe_dispute_001"  // Identification.
        connectorTransactionId = "probe_txn_001"
        disputeId = "probe_dispute_id_001"
        // evidenceDocuments: [{"evidence_type": "SERVICE_DOCUMENTATION", "file_content": "probe evidence content", "file_mime_type": "application/pdf"}]  // Collection of evidence documents.
    }.build()
    val response = client.submit_evidence(request)
    println("Dispute status: ${response.disputeStatus.name}")
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
            body = com.google.protobuf.ByteString.copyFromUtf8("{\"notificationItems\":[{\"NotificationRequestItem\":{\"pspReference\":\"probe_ref_001\",\"merchantReference\":\"probe_order_001\",\"merchantAccountCode\":\"ProbeAccount\",\"eventCode\":\"AUTHORISATION\",\"success\":\"true\",\"amount\":{\"currency\":\"USD\",\"value\":1000},\"additionalData\":{}}}]}")  // Body of the HTTP request.
        }
    }.build()
    val response = client.handle_event(request)
    println("Webhook: type=${response.eventType.name} verified=${response.sourceVerified}")
}

// Flow: PaymentService.IncrementalAuthorization
fun incrementalAuthorization(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceIncrementalAuthorizationRequest.newBuilder().apply {
        merchantAuthorizationId = "probe_auth_001"  // Identification.
        connectorTransactionId = "probe_connector_txn_001"
        amountBuilder.apply {  // new amount to be authorized (in minor currency units).
            minorAmount = 1100L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        reason = "incremental_auth_probe"  // Optional Fields.
    }.build()
    val response = client.incremental_authorization(request)
    println("Status: ${response.status.name}")
}

// Flow: EventService.ParseEvent
fun parseEvent(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = EventClient(config)
    val request = EventServiceParseRequest.newBuilder().apply {
        requestDetailsBuilder.apply {
            method = HttpMethod.HTTP_METHOD_POST  // HTTP method of the request (e.g., GET, POST).
            uri = "https://example.com/webhook"  // URI of the request.
            putAllHeaders(mapOf())  // Headers of the HTTP request.
            body = com.google.protobuf.ByteString.copyFromUtf8("{\"notificationItems\":[{\"NotificationRequestItem\":{\"pspReference\":\"probe_ref_001\",\"merchantReference\":\"probe_order_001\",\"merchantAccountCode\":\"ProbeAccount\",\"eventCode\":\"AUTHORISATION\",\"success\":\"true\",\"amount\":{\"currency\":\"USD\",\"value\":1000},\"additionalData\":{}}}]}")  // Body of the HTTP request.
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
        }
        addressBuilder.apply {
            billingAddressBuilder.apply {
            }
        }
        captureMethod = CaptureMethod.AUTOMATIC
        authType = AuthenticationType.NO_THREE_DS
        returnUrl = "https://example.com/return"
        browserInfoBuilder.apply {
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
        }
        customerBuilder.apply {
            connectorCustomerId = "probe_customer_001"  // Customer ID in the connector system.
        }
        addressBuilder.apply {
            billingAddressBuilder.apply {
            }
        }
        returnUrl = "https://example.com/return"
        customerAcceptanceBuilder.apply {
            acceptanceType = AcceptanceType.OFFLINE  // Type of acceptance (e.g., online, offline).
            acceptedAt = 0L  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        }
        authType = AuthenticationType.NO_THREE_DS
        setupFutureUsage = FutureUsage.OFF_SESSION
        browserInfoBuilder.apply {
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
        customerBuilder.apply {
            id = "cust_probe_123"  // Internal customer ID.
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
        browserInfoBuilder.apply {  // Information about the customer's browser.
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
    }.build()
    val response = client.setup_recurring(request)
    when (response.status.name) {
        "FAILED" -> throw RuntimeException("Setup failed: ${response.error.unifiedDetails.message}")
        else     -> println("Mandate stored: ${response.connectorRecurringPaymentId}")
    }
}

// Flow: PaymentService.TokenAuthorize
fun tokenAuthorize(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceTokenAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_tokenized_txn_001"
        amountBuilder.apply {
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        connectorTokenBuilder.value = "pm_1AbcXyzStripeTestToken"  // Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
        addressBuilder.apply {
            billingAddressBuilder.apply {
            }
        }
        captureMethod = CaptureMethod.AUTOMATIC
        returnUrl = "https://example.com/return"
    }.build()
    val response = client.token_authorize(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.Void
fun void(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildVoidRequest("probe_connector_txn_001")
    val response = client.void(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Void failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "processCheckoutAutocapture"
    when (flow) {
        "processCheckoutAutocapture" -> processCheckoutAutocapture(txnId)
        "processCheckoutCard" -> processCheckoutCard(txnId)
        "processRefund" -> processRefund(txnId)
        "processVoidPayment" -> processVoidPayment(txnId)
        "authorize" -> authorize(txnId)
        "capture" -> capture(txnId)
        "createClientAuthenticationToken" -> createClientAuthenticationToken(txnId)
        "createOrder" -> createOrder(txnId)
        "disputeAccept" -> disputeAccept(txnId)
        "disputeDefend" -> disputeDefend(txnId)
        "disputeSubmitEvidence" -> disputeSubmitEvidence(txnId)
        "handleEvent" -> handleEvent(txnId)
        "incrementalAuthorization" -> incrementalAuthorization(txnId)
        "parseEvent" -> parseEvent(txnId)
        "proxyAuthorize" -> proxyAuthorize(txnId)
        "proxySetupRecurring" -> proxySetupRecurring(txnId)
        "recurringCharge" -> recurringCharge(txnId)
        "refund" -> refund(txnId)
        "setupRecurring" -> setupRecurring(txnId)
        "tokenAuthorize" -> tokenAuthorize(txnId)
        "void" -> void(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: processCheckoutAutocapture, processCheckoutCard, processRefund, processVoidPayment, authorize, capture, createClientAuthenticationToken, createOrder, disputeAccept, disputeDefend, disputeSubmitEvidence, handleEvent, incrementalAuthorization, parseEvent, proxyAuthorize, proxySetupRecurring, recurringCharge, refund, setupRecurring, tokenAuthorize, void")
    }
}
