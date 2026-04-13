// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py authorizedotnet
//
// Authorizedotnet — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="authorizedotnet processCheckoutCard"

package examples.authorizedotnet

import payments.PaymentClient
import payments.RecurringPaymentClient
import payments.CustomerClient
import payments.PaymentServiceAuthorizeRequest
import payments.PaymentServiceCaptureRequest
import payments.PaymentServiceRefundRequest
import payments.PaymentServiceSetupRecurringRequest
import payments.RecurringPaymentServiceChargeRequest
import payments.PaymentServiceVoidRequest
import payments.PaymentServiceGetRequest
import payments.CustomerServiceCreateRequest
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

// Scenario: Bank Transfer (SEPA / ACH / BACS)
// Direct bank debit (Ach). Bank transfers typically use `capture_method=AUTOMATIC`.
fun processCheckoutBank(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val paymentClient = PaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val authorizeResponse = paymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"  // Identification
        amountBuilder.apply {  // The amount for the payment
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00)
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR")
        }
        paymentMethodBuilder.apply {  // Payment method to be used
            achBuilder.apply {  // Ach - Automated Clearing House
                accountNumber = "000123456789"  // Account number for ach bank debit payment
                routingNumber = "110000000"  // Routing number for ach bank debit payment
                bankAccountHolderName = "John Doe"  // Bank account holder name
            }
        }
        captureMethod = CaptureMethod.AUTOMATIC  // Method for capturing the payment
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
            connectorCustomerId = "cust_probe_123"  // Customer ID in the connector system
        }
        addressBuilder.apply {  // Address Information
            billingAddressBuilder.apply {
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Type of authentication to be used
        enrolledFor3Ds = false  // Indicates if the customer is enrolled for 3D Secure
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

// Scenario: Create Customer
// Register a customer record in the connector system. Returns a connector_customer_id that can be reused for recurring payments and tokenized card storage.
fun processCreateCustomer(txnId: String, config: ConnectorConfig = _defaultConfig): Map<String, Any?> {
    val customerClient = CustomerClient(config)

    // Step 1: Create Customer — register customer record in the connector
    val createResponse = customerClient.create(CustomerServiceCreateRequest.newBuilder().apply {
        merchantCustomerId = "cust_probe_123"  // Identification
        customerName = "John Doe"  // Name of the customer
        email = "test@example.com"  // Email address of the customer
        phoneNumber = "4155552671"  // Phone number of the customer
    }.build())

    return mapOf("customerId" to createResponse.connectorCustomerId, "error" to createResponse.error)
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

// Flow: CustomerService.Create
fun createCustomer(txnId: String) {
    val client = CustomerClient(_defaultConfig)
    val request = CustomerServiceCreateRequest.newBuilder().apply {
        merchantCustomerId = "cust_probe_123"  // Identification
        customerName = "John Doe"  // Name of the customer
        email = "test@example.com"  // Email address of the customer
        phoneNumber = "4155552671"  // Phone number of the customer
    }.build()
    val response = client.create(request)
    println("Customer: ${response.connectorCustomerId}")
}

// Flow: PaymentService.Get
fun get(txnId: String) {
    val client = PaymentClient(_defaultConfig)
    val request = buildGetRequest("probe_connector_txn_001")
    val response = client.get(request)
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
            connectorCustomerId = "cust_probe_123"  // Customer ID in the connector system
        }
        addressBuilder.apply {  // Address Information
            billingAddressBuilder.apply {
            }
        }
        authType = AuthenticationType.NO_THREE_DS  // Type of authentication to be used
        enrolledFor3Ds = false  // Indicates if the customer is enrolled for 3D Secure
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
        "processCheckoutBank" -> processCheckoutBank(txnId)
        "processRefund" -> processRefund(txnId)
        "processRecurring" -> processRecurring(txnId)
        "processVoidPayment" -> processVoidPayment(txnId)
        "processGetPayment" -> processGetPayment(txnId)
        "processCreateCustomer" -> processCreateCustomer(txnId)
        "authorize" -> authorize(txnId)
        "capture" -> capture(txnId)
        "createCustomer" -> createCustomer(txnId)
        "get" -> get(txnId)
        "recurringCharge" -> recurringCharge(txnId)
        "refund" -> refund(txnId)
        "setupRecurring" -> setupRecurring(txnId)
        "void" -> void(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: processCheckoutCard, processCheckoutAutocapture, processCheckoutBank, processRefund, processRecurring, processVoidPayment, processGetPayment, processCreateCustomer, authorize, capture, createCustomer, get, recurringCharge, refund, setupRecurring, void")
    }
}
