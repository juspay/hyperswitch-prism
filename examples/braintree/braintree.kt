// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py braintree
//
// Braintree — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="braintree processCheckoutCard"

package examples.braintree

import types.Payment.*
import types.Events.*
import types.PaymentMethods.*
import payments.PaymentClient
import payments.MerchantAuthenticationClient
import payments.EventClient
import payments.AcceptanceType
import payments.AuthenticationType
import payments.CardNetwork
import payments.Currency
import payments.FutureUsage
import payments.HttpMethod
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.BraintreeConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("capture", "create_client_authentication_token", "get", "parse_event", "proxy_setup_recurring", "refund", "reverse", "setup_recurring", "void")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setBraintree(BraintreeConfig.newBuilder()
                .setPublicKey(SecretString.newBuilder().setValue("YOUR_PUBLIC_KEY").build())
                .setPrivateKey(SecretString.newBuilder().setValue("YOUR_PRIVATE_KEY").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setMerchantAccountId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ACCOUNT_ID").build())
                .setMerchantConfigCurrency("YOUR_MERCHANT_CONFIG_CURRENCY")
                .addAllApplePaySupportedNetworks(listOf("YOUR_APPLE_PAY_SUPPORTED_NETWORKS"))
                .addAllApplePayMerchantCapabilities(listOf("YOUR_APPLE_PAY_MERCHANT_CAPABILITIES"))
                .setApplePayLabel("YOUR_APPLE_PAY_LABEL")
                .setGpayMerchantName("YOUR_GPAY_MERCHANT_NAME")
                .setGpayMerchantId("YOUR_GPAY_MERCHANT_ID")
                .addAllGpayAllowedAuthMethods(listOf("YOUR_GPAY_ALLOWED_AUTH_METHODS"))
                .addAllGpayAllowedCardNetworks(listOf("YOUR_GPAY_ALLOWED_CARD_NETWORKS"))
                .setPaypalClientId("YOUR_PAYPAL_CLIENT_ID")
                .setGpayGatewayMerchantId("YOUR_GPAY_GATEWAY_MERCHANT_ID")
                .build())
            .build()
    )
    .build()



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

private fun buildReverseRequest(connectorTransactionIdStr: String): PaymentServiceReverseRequest {
    return PaymentServiceReverseRequest.newBuilder().apply {
        merchantReverseId = "probe_reverse_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
    }.build()
}

private fun buildVoidRequest(connectorTransactionIdStr: String): PaymentServiceVoidRequest {
    return PaymentServiceVoidRequest.newBuilder().apply {
        merchantVoidId = "probe_void_001"  // Identification.
        connectorTransactionId = connectorTransactionIdStr
    }.build()
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
            body = com.google.protobuf.ByteString.copyFromUtf8("bt_signature=dummy_public_key%7Cdummy_signature&bt_payload=PG5vdGlmaWNhdGlvbj48a2luZD5kaXNwdXRlX29wZW5lZDwva2luZD48dGltZXN0YW1wPjIwMjQtMDEtMDFUMDA6MDA6MDBaPC90aW1lc3RhbXA%2BPGRpc3B1dGU%2BPGFtb3VudF9kaXNwdXRlZD4xMDAwPC9hbW91bnRfZGlzcHV0ZWQ%2BPGN1cnJlbmN5X2lzb19jb2RlPlVTRDwvY3VycmVuY3lfaXNvX2NvZGU%2BPGlkPmR1bW15X2Rpc3B1dGVfaWRfMDAxPC9pZD48a2luZD5DSEFSR0VCQUNLPC9raW5kPjxzdGF0dXM%2Bb3Blbjwvc3RhdHVzPjxyZWFzb24%2BZnJhdWQ8L3JlYXNvbj48cmVhc29uX2NvZGU%2BODM8L3JlYXNvbl9jb2RlPjx0cmFuc2FjdGlvbj48YW1vdW50PjEwLjAwPC9hbW91bnQ%2BPGlkPmR1bW15X3R4bl9pZF8wMDE8L2lkPjwvdHJhbnNhY3Rpb24%2BPC9kaXNwdXRlPjwvbm90aWZpY2F0aW9uPg%3D%3D")  // Body of the HTTP request.
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
            body = com.google.protobuf.ByteString.copyFromUtf8("bt_signature=dummy_public_key%7Cdummy_signature&bt_payload=PG5vdGlmaWNhdGlvbj48a2luZD5kaXNwdXRlX29wZW5lZDwva2luZD48dGltZXN0YW1wPjIwMjQtMDEtMDFUMDA6MDA6MDBaPC90aW1lc3RhbXA%2BPGRpc3B1dGU%2BPGFtb3VudF9kaXNwdXRlZD4xMDAwPC9hbW91bnRfZGlzcHV0ZWQ%2BPGN1cnJlbmN5X2lzb19jb2RlPlVTRDwvY3VycmVuY3lfaXNvX2NvZGU%2BPGlkPmR1bW15X2Rpc3B1dGVfaWRfMDAxPC9pZD48a2luZD5DSEFSR0VCQUNLPC9raW5kPjxzdGF0dXM%2Bb3Blbjwvc3RhdHVzPjxyZWFzb24%2BZnJhdWQ8L3JlYXNvbj48cmVhc29uX2NvZGU%2BODM8L3JlYXNvbl9jb2RlPjx0cmFuc2FjdGlvbj48YW1vdW50PjEwLjAwPC9hbW91bnQ%2BPGlkPmR1bW15X3R4bl9pZF8wMDE8L2lkPjwvdHJhbnNhY3Rpb24%2BPC9kaXNwdXRlPjwvbm90aWZpY2F0aW9uPg%3D%3D")  // Body of the HTTP request.
        }
    }.build()
    val response = client.parse_event(request)
    println("Webhook parsed: type=${response.eventType.name}")
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

// Flow: PaymentService.Refund
fun refund(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildRefundRequest("probe_connector_txn_001")
    val response = client.refund(request)
    if (response.status.name == "FAILED")
        throw RuntimeException("Refund failed: ${response.error.unifiedDetails.message}")
    println("Done: ${response.status.name}")
}

// Flow: PaymentService.Reverse
fun reverse(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildReverseRequest("probe_connector_txn_001")
    val response = client.reverse(request)
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
    val flow = args.firstOrNull() ?: "capture"
    when (flow) {
        "capture" -> capture(txnId)
        "createClientAuthenticationToken" -> createClientAuthenticationToken(txnId)
        "get" -> get(txnId)
        "handleEvent" -> handleEvent(txnId)
        "parseEvent" -> parseEvent(txnId)
        "proxySetupRecurring" -> proxySetupRecurring(txnId)
        "refund" -> refund(txnId)
        "reverse" -> reverse(txnId)
        "setupRecurring" -> setupRecurring(txnId)
        "void" -> void(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: capture, createClientAuthenticationToken, get, handleEvent, parseEvent, proxySetupRecurring, refund, reverse, setupRecurring, void")
    }
}
