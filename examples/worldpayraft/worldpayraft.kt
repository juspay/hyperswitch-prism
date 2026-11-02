// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py worldpayraft
//
// Worldpayraft — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="worldpayraft processCheckoutCard"

package examples.worldpayraft

import types.Payment.*
import types.Events.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.RecurringPaymentClient
import payments.AcceptanceType
import payments.AuthenticationType
import payments.CaptureMethod
import payments.CardNetwork
import payments.Currency
import payments.FutureUsage
import payments.PaymentMethodType
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.WorldpayraftConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("authorize", "capture", "incremental_authorization", "proxy_authorize", "proxy_setup_recurring", "recurring_charge", "refund", "reverse", "setup_recurring", "void")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setWorldpayraft(WorldpayraftConfig.newBuilder()
                .setLicense(SecretString.newBuilder().setValue("YOUR_LICENSE").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()



private fun buildAuthorizeRequest(captureMethodStr: String): PaymentServiceAuthorizeRequest {
    return PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"  // Identification.
        amountBuilder.apply {  // The amount for the payment.
            minorAmount = 1000L
            currency = Currency.USD
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

private fun buildCaptureRequest(connectorTransactionIdStr: String): PaymentServiceCaptureRequest {
    return PaymentServiceCaptureRequest.newBuilder().apply {
        merchantCaptureId = "probe_capture_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        amountToCaptureBuilder.apply {  // Capture Details.
            minorAmount = 1000L
            currency = Currency.USD
        }
        connectorFeatureDataBuilder.value = "{\"api_transaction_id\":\"0000000000000001\"}"  // Connector-specific metadata for the transaction.
    }.build()
}

private fun buildRefundRequest(connectorTransactionIdStr: String): PaymentServiceRefundRequest {
    return PaymentServiceRefundRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        paymentAmount = 1000L  // Amount Information.
        refundAmountBuilder.apply {
            minorAmount = 1000L
            currency = Currency.USD
        }
        reason = "customer_request"  // Reason for the refund.
        paymentMethodBuilder.apply {  // Original payment method, for connectors that require it again on refund (e.g. Worldpay RAFT).
            cardWithNoCvcBuilder.apply {  // Card payment where CVC is not required.
                cardNumberBuilder.value = "4111111111111111"  // Card Identification.
                cardExpMonthBuilder.value = "03"
                cardExpYearBuilder.value = "2030"
                cardHolderNameBuilder.value = "John Doe"  // Cardholder Information.
                cardNetwork = CardNetwork.VISA
            }
        }
    }.build()
}

private fun buildReverseRequest(connectorTransactionIdStr: String): PaymentServiceReverseRequest {
    return PaymentServiceReverseRequest.newBuilder().apply {
        merchantReverseId = "probe_reverse_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        amountBuilder.apply {  // Original payment amount and currency.
            minorAmount = 1000L
            currency = Currency.USD
        }
        paymentMethodBuilder.apply {  // Original payment method for connectors that require PAN and expiry again.
            cardWithNoCvcBuilder.apply {  // Card payment where CVC is not required.
                cardNumberBuilder.value = "4111111111111111"  // Card Identification.
                cardExpMonthBuilder.value = "03"
                cardExpYearBuilder.value = "2030"
                cardNetwork = CardNetwork.VISA
            }
        }
        captureMethod = CaptureMethod.MANUAL  // Original payment's capture method, not the reversal's disposition. Worldpay RAFT reverses MANUAL completions and AUTOMATIC purchases at different endpoints.
    }.build()
}

private fun buildVoidRequest(connectorTransactionIdStr: String): PaymentServiceVoidRequest {
    return PaymentServiceVoidRequest.newBuilder().apply {
        merchantVoidId = "probe_void_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L
            currency = Currency.USD
        }
        connectorFeatureDataBuilder.value = "{\"api_transaction_id\":\"0000000000000001\"}"  // Connector-specific metadata for the transaction.
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
    val request = buildCaptureRequest("C|123456|123456789012|000001")
    val response = client.capture(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Capture failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}

// Flow: PaymentService.IncrementalAuthorization
fun incrementalAuthorization(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceIncrementalAuthorizationRequest.newBuilder().apply {
        merchantAuthorizationId = "probe_auth_001"  // Identification.
        connectorTransactionId = "C|123456|123456789012|000001"
        amountBuilder.apply {  // New total authorized amount, including this increase.
            minorAmount = 1100L
            currency = Currency.USD
        }
        reason = "incremental_auth_probe"  // Optional Fields.
        additionalAmountBuilder.apply {  // Increase over the current authorized total. Must use the same currency as amount.
            minorAmount = 100L
            currency = Currency.USD
        }
        networkTransactionId = "263010131742906"  // Network transaction ID returned by the original authorization.
        paymentMethodBuilder.apply {  // Original payment method, for connectors that require it again on an increase (e.g. Worldpay RAFT).
            cardWithNoCvcBuilder.apply {  // Card payment where CVC is not required.
                cardNumberBuilder.value = "4111111111111111"  // Card Identification.
                cardExpMonthBuilder.value = "03"
                cardExpYearBuilder.value = "2030"
                cardNetwork = CardNetwork.VISA
            }
        }
        connectorFeatureDataBuilder.value = "{\"api_transaction_id\":\"probe_auth_001\",\"original_authorized_amount\":{\"amount\":1000,\"currency\":\"USD\"},\"terminal_data\":{\"EntryMode\":\"KEYED\",\"POSConditionCode\":\"59\",\"TerminalEntryCap\":\"0\"},\"ecommerce_data\":{\"E-commerceIndicator\":\"07\"},\"proc_flags_indicators\":{\"EstimatedAuth\":\"Y\"},\"brand_specific_data\":{\"VisaSpecificData\":{\"VisaTransactionId\":\"263010131742906\"}}}"
    }.build()
    val response = client.incremental_authorization(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.ProxyAuthorize
fun proxyAuthorize(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceProxyAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_proxy_txn_001"
        amountBuilder.apply {
            minorAmount = 1000L
            currency = Currency.USD
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
            minorAmount = 0L
            currency = Currency.USD
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
            minorAmount = 1000L
            currency = Currency.USD
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
    val request = buildRefundRequest("C|123456|123456789012|000001")
    val response = client.refund(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Refund failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}

// Flow: PaymentService.Reverse
fun reverse(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildReverseRequest("C|123456|123456789012|000001")
    val response = client.reverse(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.SetupRecurring
fun setupRecurring(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceSetupRecurringRequest.newBuilder().apply {
        merchantRecurringPaymentId = "probe_mandate_001"  // Identification.
        amountBuilder.apply {  // Mandate Details.
            minorAmount = 0L
            currency = Currency.USD
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

// Flow: PaymentService.Void
fun void(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildVoidRequest("C|123456|123456789012|000001")
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
        "incrementalAuthorization" -> incrementalAuthorization(txnId)
        "proxyAuthorize" -> proxyAuthorize(txnId)
        "proxySetupRecurring" -> proxySetupRecurring(txnId)
        "recurringCharge" -> recurringCharge(txnId)
        "refund" -> refund(txnId)
        "reverse" -> reverse(txnId)
        "setupRecurring" -> setupRecurring(txnId)
        "void" -> void(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: processCheckoutAutocapture, processCheckoutCard, processRefund, processVoidPayment, authorize, capture, incrementalAuthorization, proxyAuthorize, proxySetupRecurring, recurringCharge, refund, reverse, setupRecurring, void")
    }
}
