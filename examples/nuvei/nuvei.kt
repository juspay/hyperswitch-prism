// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py nuvei
//
// Nuvei — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="nuvei processCheckoutCard"

package examples.nuvei

import types.Payment.*
import types.Events.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.PaymentMethodAuthenticationClient
import payments.MerchantAuthenticationClient
import payments.EventClient
import payments.RecurringPaymentClient
import payments.RefundClient
import payments.AcceptanceType
import payments.AuthenticationType
import payments.CaptureMethod
import payments.CountryAlpha2
import payments.Currency
import payments.FutureUsage
import payments.HttpMethod
import payments.PaymentChannel
import payments.PaymentMethodType
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.NuveiConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("authenticate", "authorize", "capture", "create_client_authentication_token", "create_order", "create_server_session_authentication_token", "get", "parse_event", "post_authenticate", "pre_authenticate", "recurring_charge", "refund", "refund_get", "setup_recurring", "void")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setNuvei(NuveiConfig.newBuilder()
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setMerchantSiteId(SecretString.newBuilder().setValue("YOUR_MERCHANT_SITE_ID").build())
                .setMerchantSecret(SecretString.newBuilder().setValue("YOUR_MERCHANT_SECRET").build())
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
            id = "cust_probe_123"  // Internal customer ID.
        }
        addressBuilder.apply {  // Address Information.
            shippingAddressBuilder.apply {
                firstNameBuilder.value = "John"  // Personal Information.
                lastNameBuilder.value = "Doe"
                line1Builder.value = "123 Main St"  // Address Details.
                cityBuilder.value = "Seattle"
                zipCodeBuilder.value = "98101"
                countryAlpha2Code = CountryAlpha2.US
            }
            billingAddressBuilder.apply {
                countryAlpha2Code = CountryAlpha2.US
                emailBuilder.value = "test@example.com"  // Contact Information.
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Authentication Details.
        authenticationDataBuilder.apply {
            eci = "05"  // Electronic Commerce Indicator (ECI) from 3DS.
            cavv = "AAAAAAAAAA=="  // Cardholder Authentication Verification Value (CAVV).
            dsTransactionId = "probe-ds-txn-001"  // Directory Server Transaction ID (DS Trans ID).
        }
        returnUrl = "https://example.com/return"  // URLs for Redirection and Webhooks.
        sessionToken = "probe_session_token"  // Session and Token Information.
        setupFutureUsage = FutureUsage.OFF_SESSION  // Behavioral Flags and Preferences.
        enablePartialAuthorization = true
        customerAcceptanceBuilder.apply {  // Contextual Information.
            acceptanceType = AcceptanceType.OFFLINE  // Type of acceptance (e.g., online, offline).
            acceptedAt = 0L  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        }
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
        paymentChannel = PaymentChannel.MAIL_ORDER
        billingDescriptorBuilder.apply {  // Statement Descriptor.
            nameBuilder.value = "Probe Merchant"  // Customer's billing name.
            phoneBuilder.value = "4155550132"  // Customer's billing phone number.
        }
        l2L3DataBuilder.apply {  // Level 2 / Level 3 data for enhanced payment processing.
            orderInfoBuilder.apply {  // Order-level information.
                // orderDetails: [{"product_name": "ProbeProduct", "quantity": 1, "amount": 1000, "total_tax_amount": 100, "product_id": "probe_product_001"}]  // Line items for the order.
                shippingCost = 100L  // Shipping cost for the order (in minor currency units).
            }
            taxInfoBuilder.apply {  // Tax information.
                orderTaxAmount = 100L  // Total tax amount for the order (in minor currency units).
            }
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

private fun buildVoidRequest(connectorTransactionIdStr: String): PaymentServiceVoidRequest {
    return PaymentServiceVoidRequest.newBuilder().apply {
        merchantVoidId = "probe_void_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
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

// Flow: PaymentMethodAuthenticationService.Authenticate
fun authenticate(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentMethodAuthenticationClient(config)
    val request = PaymentMethodAuthenticationServiceAuthenticateRequest.newBuilder().apply {
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        paymentMethodBuilder.apply {  // Payment Method.
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
                countryAlpha2Code = CountryAlpha2.US
                emailBuilder.value = "test@example.com"  // Contact Information.
            }
        }
        authenticationDataBuilder.apply {  // Authentication Details.
            connectorTransactionId = "probe_init_txn_001"  // Transaction identifier generated by the 3DS system.
        }
        returnUrl = "https://example.com/3ds-return"  // URLs for Redirection.
        continueRedirectionUrl = "https://example.com/3ds-continue"
        browserInfoBuilder.apply {  // Contextual Information.
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
        stateBuilder.apply {  // State Information.
            accessTokenBuilder.apply {  // Access token obtained from connector.
                tokenBuilder.value = "probe_session_token"  // The token string.
            }
        }
    }.build()
    val response = client.authenticate(request)
    println("Status: ${response.status.name}")
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
        paymentBuilder.apply {
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

// Flow: MerchantAuthenticationService.CreateServerSessionAuthenticationToken
fun createServerSessionAuthenticationToken(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = MerchantAuthenticationClient(config)
    val request = MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest.newBuilder().apply {
        paymentBuilder.apply {  // PayoutSessionContext payout = 6; // future FrmSessionContext frm = 7; // future.
            amountBuilder.apply {
                minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
                currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
            }
        }
    }.build()
    val response = client.create_server_session_authentication_token(request)
    println("Session token: ${response.sessionToken} (statusCode=${response.statusCode})")
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
            body = com.google.protobuf.ByteString.copyFromUtf8("ppp_status=OK&PPP_TransactionID=547&TransactionID=2110000000004302220&Status=APPROVED&transactionType=Sale&totalAmount=115&currency=USD&responseTimeStamp=2020-03-14.16%3A22%3A34&productId=Your+Product")  // Body of the HTTP request.
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
            body = com.google.protobuf.ByteString.copyFromUtf8("ppp_status=OK&PPP_TransactionID=547&TransactionID=2110000000004302220&Status=APPROVED&transactionType=Sale&totalAmount=115&currency=USD&responseTimeStamp=2020-03-14.16%3A22%3A34&productId=Your+Product")  // Body of the HTTP request.
        }
    }.build()
    val response = client.parse_event(request)
    println("Webhook parsed: type=${response.eventType.name}")
}

// Flow: PaymentMethodAuthenticationService.PostAuthenticate
fun postAuthenticate(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentMethodAuthenticationClient(config)
    val request = PaymentMethodAuthenticationServicePostAuthenticateRequest.newBuilder().apply {

    }.build()
    val response = client.post_authenticate(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentMethodAuthenticationService.PreAuthenticate
fun preAuthenticate(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentMethodAuthenticationClient(config)
    val request = PaymentMethodAuthenticationServicePreAuthenticateRequest.newBuilder().apply {
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        paymentMethodBuilder.apply {  // Payment Method.
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
                countryAlpha2Code = CountryAlpha2.US
                emailBuilder.value = "test@example.com"  // Contact Information.
            }
        }
        enrolledFor3Ds = false  // Authentication Details.
        returnUrl = "https://example.com/3ds-return"  // URLs for Redirection.
        browserInfoBuilder.apply {  // Contextual Information.
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
        stateBuilder.apply {  // State Information.
            accessTokenBuilder.apply {  // Access token obtained from connector.
                tokenBuilder.value = "probe_session_token"  // The token string.
            }
        }
    }.build()
    val response = client.pre_authenticate(request)
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
            cardBuilder.apply {  // Generic card payment.
                cardNumberBuilder.value = "4111111111111111"  // Card Identification.
                cardExpMonthBuilder.value = "03"
                cardExpYearBuilder.value = "2030"
                cardCvcBuilder.value = "737"
                cardHolderNameBuilder.value = "John Doe"  // Cardholder Information.
            }
        }
        returnUrl = "https://example.com/recurring-return"
        addressBuilder.apply {  // Address Information.
            billingAddressBuilder.apply {
                countryAlpha2Code = CountryAlpha2.US
                emailBuilder.value = "test@example.com"  // Contact Information.
            }
        }
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
        stateBuilder.apply {  // State Information.
            accessTokenBuilder.apply {  // Access token obtained from connector.
                tokenBuilder.value = "probe_session_token"  // The token string.
            }
        }
        customerBuilder.apply {  // Customer Information.
            id = "cust_probe_123"  // Internal customer ID.
        }
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
        customerBuilder.apply {
            id = "cust_probe_123"  // Internal customer ID.
        }
        addressBuilder.apply {  // Address Information.
            billingAddressBuilder.apply {
                firstNameBuilder.value = "John"  // Personal Information.
                lastNameBuilder.value = "Doe"
                countryAlpha2Code = CountryAlpha2.US
                emailBuilder.value = "test@example.com"  // Contact Information.
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Type of authentication to be used.
        enrolledFor3Ds = false  // Indicates if the customer is enrolled for 3D Secure.
        returnUrl = "https://example.com/mandate-return"  // URL to redirect after setup.
        sessionToken = "probe_session_token"  // Session token, if applicable.
        setupFutureUsage = FutureUsage.OFF_SESSION  // Indicates future usage intention.
        requestIncrementalAuthorization = false  // Indicates if incremental authorization is requested.
        customerAcceptanceBuilder.apply {  // Details of customer acceptance.
            acceptanceType = AcceptanceType.OFFLINE  // Type of acceptance (e.g., online, offline).
            acceptedAt = 0L  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        }
        browserInfoBuilder.apply {  // Information about the customer's browser.
            ipAddress = "1.2.3.4"  // Device Information.
        }
    }.build()
    val response = client.setup_recurring(request)
    when (response.status.name) {
        "FAILED" -> throw RuntimeException("Setup failed: ${response.error.unifiedDetails.message}")
        else     -> println("Mandate stored: ${response.connectorRecurringPaymentId}")
    }
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
        "processGetPayment" -> processGetPayment(txnId)
        "authenticate" -> authenticate(txnId)
        "authorize" -> authorize(txnId)
        "capture" -> capture(txnId)
        "createClientAuthenticationToken" -> createClientAuthenticationToken(txnId)
        "createOrder" -> createOrder(txnId)
        "createServerSessionAuthenticationToken" -> createServerSessionAuthenticationToken(txnId)
        "get" -> get(txnId)
        "handleEvent" -> handleEvent(txnId)
        "parseEvent" -> parseEvent(txnId)
        "postAuthenticate" -> postAuthenticate(txnId)
        "preAuthenticate" -> preAuthenticate(txnId)
        "recurringCharge" -> recurringCharge(txnId)
        "refund" -> refund(txnId)
        "refundGet" -> refundGet(txnId)
        "setupRecurring" -> setupRecurring(txnId)
        "void" -> void(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: processCheckoutAutocapture, processCheckoutCard, processRefund, processVoidPayment, processGetPayment, authenticate, authorize, capture, createClientAuthenticationToken, createOrder, createServerSessionAuthenticationToken, get, handleEvent, parseEvent, postAuthenticate, preAuthenticate, recurringCharge, refund, refundGet, setupRecurring, void")
    }
}
