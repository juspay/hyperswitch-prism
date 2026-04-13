// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py cybersource
//
// Cybersource — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="cybersource processCheckoutCard"

package examples.cybersource

import payments.PaymentClient
import payments.RecurringPaymentClient
import payments.PaymentMethodAuthenticationClient
import payments.PaymentServiceAuthorizeRequest
import payments.PaymentServiceCaptureRequest
import payments.PaymentServiceRefundRequest
import payments.PaymentServiceSetupRecurringRequest
import payments.RecurringPaymentServiceChargeRequest
import payments.PaymentServiceVoidRequest
import payments.PaymentServiceGetRequest
import payments.PaymentMethodAuthenticationServicePreAuthenticateRequest
import payments.AcceptanceType
import payments.AuthenticationType
import payments.CaptureMethod
import payments.Currency
import payments.FutureUsage
import payments.PaymentMethodType
import payments.TokenPaymentMethodType
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
            cardBuilder.apply {  // Generic card payment
                cardNumber = "4111111111111111"  // Card Identification
                cardExpMonth = "03"
                cardExpYear = "2030"
                cardCvc = "737"
                cardHolderName = "John Doe"  // Cardholder Information
            }
        }
        captureMethod = CaptureMethod.valueOf(captureMethodStr)  // Method for capturing the payment
        customerBuilder.apply {  // Customer Information
            email = "test@example.com"  // Customer's email address
        }
        addressBuilder.apply {  // Address Information
            billingAddressBuilder.apply {
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Authentication Details
        returnUrl = "https://example.com/return"  // URLs for Redirection and Webhooks
    }.build()
}

private fun buildCaptureRequest(connectorTransactionIdStr: String): PaymentServiceCaptureRequest {
    return PaymentServiceCaptureRequest.newBuilder().apply {
        merchantCaptureId = "probe_capture_001"  // Identification
        connectorTransactionId = connectorTransactionIdStr
        amountToCaptureBuilder.apply {  // Capture Details
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00)
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR")
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
    }.build()
}

private fun buildVoidRequest(connectorTransactionIdStr: String): PaymentServiceVoidRequest {
    return PaymentServiceVoidRequest.newBuilder().apply {
        merchantVoidId = "probe_void_001"  // Identification
        connectorTransactionId = connectorTransactionIdStr
        cancellationReason = "requested_by_customer"  // Void Details
        amountBuilder.apply {  // Amount Information
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00)
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR")
        }
    }.build()
}

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your connector config here
    .build()


// Scenario: Card Payment (Authorize + Capture)
// Reserve funds with Authorize, then settle with a separate Capture call. Use for physical goods or delayed fulfillment where capture happens later.
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

// Scenario: Card Payment (Automatic Capture)
// Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.
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

// Scenario: Wallet Payment (Google Pay / Apple Pay)
// Wallet payments pass an encrypted token from the browser/device SDK. Pass the token blob directly — do not decrypt client-side.
fun processCheckoutWallet(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val authorizeResponse = paymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"  // Identification
        amountBuilder.apply {  // The amount for the payment
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00)
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR")
        }
        paymentMethodBuilder.apply {  // Payment method to be used
            googlePayBuilder.apply {  // Google Pay
                type = "CARD"  // Type of payment method
                description = "Visa 1111"  // User-facing description of the payment method
                infoBuilder.apply {
                    cardNetwork = "VISA"  // Card network name
                    cardDetails = "1111"  // Card details (usually last 4 digits)
                }
                tokenizationDataBuilder.apply {
                    encryptedDataBuilder.apply {  // Encrypted Google Pay payment data
                        tokenType = "PAYMENT_GATEWAY"  // The type of the token
                        token = "{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}"  // Token generated for the wallet
                    }
                }
            }
        }
        captureMethod = CaptureMethod.AUTOMATIC  // Method for capturing the payment
        customerBuilder.apply {  // Customer Information
            email = "test@example.com"  // Customer's email address
        }
        addressBuilder.apply {  // Address Information
            billingAddressBuilder.apply {
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Authentication Details
        returnUrl = "https://example.com/return"  // URLs for Redirection and Webhooks
    }.build())

    when (authorizeResponse.status.name) {
        "FAILED"  -> throw RuntimeException("Payment failed: ${authorizeResponse.error.unifiedDetails.message}")
        "PENDING" -> return mapOf("status" to "PENDING")  // await webhook before proceeding
    }

    return mapOf("status" to authorizeResponse.status.name, "transactionId" to authorizeResponse.connectorTransactionId, "error" to authorizeResponse.error)
}

// Scenario: Refund a Payment
// Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.
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

// Scenario: Recurring / Mandate Payments
// Store a payment mandate with SetupRecurring, then charge it repeatedly with RecurringPaymentService.Charge without requiring customer action.
fun processRecurring(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)
    val recurringPaymentClient = RecurringPaymentClient(config)

    // Step 1: Setup Recurring — store the payment mandate
    val setupResponse = paymentClient.setup_recurring(PaymentServiceSetupRecurringRequest.newBuilder().apply {
        merchantRecurringPaymentId = "probe_mandate_001"  // Identification
        amountBuilder.apply {  // Mandate Details
            minorAmount = 0L  // Amount in minor units (e.g., 1000 = $10.00)
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR")
        }
        paymentMethodBuilder.apply {
            cardBuilder.apply {  // Generic card payment
                cardNumber = "4111111111111111"  // Card Identification
                cardExpMonth = "03"
                cardExpYear = "2030"
                cardCvc = "737"
                cardHolderName = "John Doe"  // Cardholder Information
            }
        }
        customerBuilder.apply {
            email = "test@example.com"  // Customer's email address
        }
        addressBuilder.apply {  // Address Information
            billingAddressBuilder.apply {
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Type of authentication to be used
        enrolledFor3Ds = false  // Indicates if the customer is enrolled for 3D Secure
        metadata = "{\"reference_id\":\"probe_ref_001\",\"connector_request_id\":\"probe_req_001\",\"transaction_id\":\"probe_txn_001\"}"  // Additional metadata for the connector
        returnUrl = "https://example.com/mandate-return"  // URL to redirect after setup
        setupFutureUsage = FutureUsage.OFF_SESSION  // Indicates future usage intention
        requestIncrementalAuthorization = false  // Indicates if incremental authorization is requested
        customerAcceptanceBuilder.apply {  // Details of customer acceptance
            acceptanceType = AcceptanceType.OFFLINE  // Type of acceptance (e.g., online, offline).
            acceptedAt = 0L  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        }
    }.build())

    if (setupResponse.status.name == "FAILED")
        throw RuntimeException("Setup failed: ${setupResponse.error.unifiedDetails.message}")

    // Step 2: Recurring Charge — charge against the stored mandate
    val recurringResponse = recurringPaymentClient.charge(RecurringPaymentServiceChargeRequest.newBuilder().apply {
        amountBuilder.apply {  // Amount Information
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00)
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR")
        }
        returnUrl = "https://example.com/recurring-return"
        connectorCustomerId = "cust_probe_123"
        offSession = true  // Behavioral Flags and Preferences
        connectorRecurringPaymentIdBuilder.apply {
            connectorMandateIdBuilder.apply {
                connectorMandateId = setupResponse.mandateReference.connectorMandateId.connectorMandateId  // from SetupRecurring
            }
        }
    }.build())

    if (recurringResponse.status.name == "FAILED")
        throw RuntimeException("Recurring Charge failed: ${recurringResponse.error.unifiedDetails.message}")

    return mapOf("status" to recurringResponse.status.name, "transactionId" to (recurringResponse.connectorTransactionId ?: ""), "error" to recurringResponse.error)
}

// Scenario: Void a Payment
// Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.
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
// Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.
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

// Flow: PaymentService.Capture
fun capture(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildCaptureRequest("probe_connector_txn_001")
    val response = client.capture(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Capture failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}

// Flow: PaymentService.Get
fun get(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildGetRequest("probe_connector_txn_001")
    val response = client.get(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentMethodAuthenticationService.PreAuthenticate
fun preAuthenticate(txnId: String) {
    val client = PaymentMethodAuthenticationClient(_defaultConfig)
    val request = PaymentMethodAuthenticationServicePreAuthenticateRequest.newBuilder().apply {
        amountBuilder.apply {  // Amount Information
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00)
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR")
        }
        paymentMethodBuilder.apply {  // Payment Method
            cardBuilder.apply {  // Generic card payment
                cardNumber = "4111111111111111"  // Card Identification
                cardExpMonth = "03"
                cardExpYear = "2030"
                cardCvc = "737"
                cardHolderName = "John Doe"  // Cardholder Information
            }
        }
        addressBuilder.apply {  // Address Information
            billingAddressBuilder.apply {
            }
        }
        enrolledFor3Ds = false  // Authentication Details
        returnUrl = "https://example.com/3ds-return"  // URLs for Redirection
    }.build()
    val response = client.pre_authenticate(request)
    println("Status: ${response.status.name}")
}

// Flow: RecurringPaymentService.Charge
fun recurringCharge(txnId: String) {
    val client = RecurringPaymentClient(_defaultConfig)
    val request = RecurringPaymentServiceChargeRequest.newBuilder().apply {
        connectorRecurringPaymentIdBuilder.apply {  // Reference to existing mandate
            connectorMandateIdBuilder.apply {  // mandate_id sent by the connector
                connectorMandateId = "probe-mandate-123"
            }
        }
        amountBuilder.apply {  // Amount Information
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00)
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR")
        }
        paymentMethodBuilder.apply {  // Optional payment Method Information (for network transaction flows)
            token = TokenPaymentMethodType.probe_pm_token  // Payment tokens
        }
        returnUrl = "https://example.com/recurring-return"
        connectorCustomerId = "cust_probe_123"
        paymentMethodType = PaymentMethodType.PAY_PAL
        offSession = true  // Behavioral Flags and Preferences
    }.build()
    val response = client.charge(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Recurring_Charge failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
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

// Flow: PaymentService.SetupRecurring
fun setupRecurring(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = PaymentServiceSetupRecurringRequest.newBuilder().apply {
        merchantRecurringPaymentId = "probe_mandate_001"  // Identification
        amountBuilder.apply {  // Mandate Details
            minorAmount = 0L  // Amount in minor units (e.g., 1000 = $10.00)
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR")
        }
        paymentMethodBuilder.apply {
            cardBuilder.apply {  // Generic card payment
                cardNumber = "4111111111111111"  // Card Identification
                cardExpMonth = "03"
                cardExpYear = "2030"
                cardCvc = "737"
                cardHolderName = "John Doe"  // Cardholder Information
            }
        }
        customerBuilder.apply {
            email = "test@example.com"  // Customer's email address
        }
        addressBuilder.apply {  // Address Information
            billingAddressBuilder.apply {
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Type of authentication to be used
        enrolledFor3Ds = false  // Indicates if the customer is enrolled for 3D Secure
        metadata = "{\"reference_id\":\"probe_ref_001\",\"connector_request_id\":\"probe_req_001\",\"transaction_id\":\"probe_txn_001\"}"  // Additional metadata for the connector
        returnUrl = "https://example.com/mandate-return"  // URL to redirect after setup
        setupFutureUsage = FutureUsage.OFF_SESSION  // Indicates future usage intention
        requestIncrementalAuthorization = false  // Indicates if incremental authorization is requested
        customerAcceptanceBuilder.apply {  // Details of customer acceptance
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
    val flow = args.firstOrNull() ?: "processCheckoutCard"
    when (flow) {
        "processCheckoutCard" -> processCheckoutCard(txnId)
        "processCheckoutAutocapture" -> processCheckoutAutocapture(txnId)
        "processCheckoutWallet" -> processCheckoutWallet(txnId)
        "processRefund" -> processRefund(txnId)
        "processRecurring" -> processRecurring(txnId)
        "processVoidPayment" -> processVoidPayment(txnId)
        "processGetPayment" -> processGetPayment(txnId)
        "authorize" -> authorize(txnId)
        "capture" -> capture(txnId)
        "get" -> get(txnId)
        "preAuthenticate" -> preAuthenticate(txnId)
        "recurringCharge" -> recurringCharge(txnId)
        "refund" -> refund(txnId)
        "setupRecurring" -> setupRecurring(txnId)
        "void" -> void(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: processCheckoutCard, processCheckoutAutocapture, processCheckoutWallet, processRefund, processRecurring, processVoidPayment, processGetPayment, authorize, capture, get, preAuthenticate, recurringCharge, refund, setupRecurring, void")
    }
}
