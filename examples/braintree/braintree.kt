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
import payments.PaymentMethodAuthenticationClient
import payments.RecurringPaymentClient
import payments.RefundClient
import payments.AcceptanceType
import payments.CaptureMethod
import payments.Currency
import payments.FutureUsage
import payments.HttpMethod
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.BraintreeConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("capture", "create_client_authentication_token", "get", "parse_event", "post_authenticate", "pre_authenticate", "recurring_revoke", "refund", "refund_get", "reverse", "token_authorize", "token_setup_recurring", "void")

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
            body = com.google.protobuf.ByteString.copyFromUtf8("bt_signature=dummy_public_key%7Cdummy_signature&bt_payload=PG5vdGlmaWNhdGlvbj48dGltZXN0YW1wIHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdGltZXN0YW1wPjxraW5kPmRpc3B1dGVfb3BlbmVkPC9raW5kPjxzdWJqZWN0PjxkaXNwdXRlPjxpZD5kdW1teV9kaXNwdXRlX2lkXzAwMTwvaWQ%2BPGFtb3VudD4xMC4wMDwvYW1vdW50PjxhbW91bnQtZGlzcHV0ZWQ%2BMTAuMDA8L2Ftb3VudC1kaXNwdXRlZD48YW1vdW50LXdvbiBuaWw9InRydWUiLz48Y2FzZS1udW1iZXI%2BQ0FTRS0wMDE8L2Nhc2UtbnVtYmVyPjxjcmVhdGVkLWF0IHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvY3JlYXRlZC1hdD48Y3VycmVuY3ktaXNvLWNvZGU%2BVVNEPC9jdXJyZW5jeS1pc28tY29kZT48Zm9yd2FyZGVkLWNvbW1lbnRzIG5pbD0idHJ1ZSIvPjxraW5kPmNoYXJnZWJhY2s8L2tpbmQ%2BPG1lcmNoYW50LWFjY291bnQtaWQ%2BZHVtbXlfbWVyY2hhbnRfYWNjb3VudDwvbWVyY2hhbnQtYWNjb3VudC1pZD48cmVhc29uPmZyYXVkPC9yZWFzb24%2BPHJlYXNvbi1jb2RlIG5pbD0idHJ1ZSIvPjxyZWNlaXZlZC1kYXRlIHR5cGU9ImRhdGUiPjIwMjYtMDktMTY8L3JlY2VpdmVkLWRhdGU%2BPHJlZmVyZW5jZS1udW1iZXI%2BUkVGLTAwMTwvcmVmZXJlbmNlLW51bWJlcj48cmVwbHktYnktZGF0ZSB0eXBlPSJkYXRlIj4yMDI2LTA5LTMwPC9yZXBseS1ieS1kYXRlPjxzdGF0dXM%2Bb3Blbjwvc3RhdHVzPjx1cGRhdGVkLWF0IHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdXBkYXRlZC1hdD48c3RhdHVzLWhpc3RvcnkgdHlwZT0iYXJyYXkiPjxzdGF0dXMtaGlzdG9yeT48c3RhdHVzPm9wZW48L3N0YXR1cz48dGltZXN0YW1wIHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdGltZXN0YW1wPjwvc3RhdHVzLWhpc3Rvcnk%2BPC9zdGF0dXMtaGlzdG9yeT48ZXZpZGVuY2UgdHlwZT0iYXJyYXkiLz48dHJhbnNhY3Rpb24%2BPGlkPmR1bW15X3R4bl9pZF8wMDE8L2lkPjxhbW91bnQ%2BMTAuMDA8L2Ftb3VudD48b3JkZXItaWQ%2BZHVtbXlfb3JkZXJfMDAxPC9vcmRlci1pZD48cGF5bWVudC1pbnN0cnVtZW50LXR5cGU%2BY3JlZGl0X2NhcmQ8L3BheW1lbnQtaW5zdHJ1bWVudC10eXBlPjwvdHJhbnNhY3Rpb24%2BPGRhdGUtb3BlbmVkIHR5cGU9ImRhdGUiPjIwMjYtMDktMTY8L2RhdGUtb3BlbmVkPjwvZGlzcHV0ZT48L3N1YmplY3Q%2BPC9ub3RpZmljYXRpb24%2B")  // Body of the HTTP request.
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
            body = com.google.protobuf.ByteString.copyFromUtf8("bt_signature=dummy_public_key%7Cdummy_signature&bt_payload=PG5vdGlmaWNhdGlvbj48dGltZXN0YW1wIHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdGltZXN0YW1wPjxraW5kPmRpc3B1dGVfb3BlbmVkPC9raW5kPjxzdWJqZWN0PjxkaXNwdXRlPjxpZD5kdW1teV9kaXNwdXRlX2lkXzAwMTwvaWQ%2BPGFtb3VudD4xMC4wMDwvYW1vdW50PjxhbW91bnQtZGlzcHV0ZWQ%2BMTAuMDA8L2Ftb3VudC1kaXNwdXRlZD48YW1vdW50LXdvbiBuaWw9InRydWUiLz48Y2FzZS1udW1iZXI%2BQ0FTRS0wMDE8L2Nhc2UtbnVtYmVyPjxjcmVhdGVkLWF0IHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvY3JlYXRlZC1hdD48Y3VycmVuY3ktaXNvLWNvZGU%2BVVNEPC9jdXJyZW5jeS1pc28tY29kZT48Zm9yd2FyZGVkLWNvbW1lbnRzIG5pbD0idHJ1ZSIvPjxraW5kPmNoYXJnZWJhY2s8L2tpbmQ%2BPG1lcmNoYW50LWFjY291bnQtaWQ%2BZHVtbXlfbWVyY2hhbnRfYWNjb3VudDwvbWVyY2hhbnQtYWNjb3VudC1pZD48cmVhc29uPmZyYXVkPC9yZWFzb24%2BPHJlYXNvbi1jb2RlIG5pbD0idHJ1ZSIvPjxyZWNlaXZlZC1kYXRlIHR5cGU9ImRhdGUiPjIwMjYtMDktMTY8L3JlY2VpdmVkLWRhdGU%2BPHJlZmVyZW5jZS1udW1iZXI%2BUkVGLTAwMTwvcmVmZXJlbmNlLW51bWJlcj48cmVwbHktYnktZGF0ZSB0eXBlPSJkYXRlIj4yMDI2LTA5LTMwPC9yZXBseS1ieS1kYXRlPjxzdGF0dXM%2Bb3Blbjwvc3RhdHVzPjx1cGRhdGVkLWF0IHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdXBkYXRlZC1hdD48c3RhdHVzLWhpc3RvcnkgdHlwZT0iYXJyYXkiPjxzdGF0dXMtaGlzdG9yeT48c3RhdHVzPm9wZW48L3N0YXR1cz48dGltZXN0YW1wIHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdGltZXN0YW1wPjwvc3RhdHVzLWhpc3Rvcnk%2BPC9zdGF0dXMtaGlzdG9yeT48ZXZpZGVuY2UgdHlwZT0iYXJyYXkiLz48dHJhbnNhY3Rpb24%2BPGlkPmR1bW15X3R4bl9pZF8wMDE8L2lkPjxhbW91bnQ%2BMTAuMDA8L2Ftb3VudD48b3JkZXItaWQ%2BZHVtbXlfb3JkZXJfMDAxPC9vcmRlci1pZD48cGF5bWVudC1pbnN0cnVtZW50LXR5cGU%2BY3JlZGl0X2NhcmQ8L3BheW1lbnQtaW5zdHJ1bWVudC10eXBlPjwvdHJhbnNhY3Rpb24%2BPGRhdGUtb3BlbmVkIHR5cGU9ImRhdGUiPjIwMjYtMDktMTY8L2RhdGUtb3BlbmVkPjwvZGlzcHV0ZT48L3N1YmplY3Q%2BPC9ub3RpZmljYXRpb24%2B")  // Body of the HTTP request.
        }
    }.build()
    val response = client.parse_event(request)
    println("Webhook parsed: type=${response.eventType.name}")
}

// Flow: PaymentMethodAuthenticationService.PostAuthenticate
fun postAuthenticate(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentMethodAuthenticationClient(config)
    val request = PaymentMethodAuthenticationServicePostAuthenticateRequest.newBuilder().apply {
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
            }
        }
        connectorOrderReferenceId = "probe_order_ref_001"
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
            }
        }
        enrolledFor3Ds = false  // Authentication Details.
        returnUrl = "https://example.com/3ds-return"  // URLs for Redirection.
    }.build()
    val response = client.pre_authenticate(request)
    println("Status: ${response.status.name}")
}

// Flow: RecurringPaymentService.Revoke
fun recurringRevoke(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = RecurringPaymentClient(config)
    val request = RecurringPaymentServiceRevokeRequest.newBuilder().apply {
        merchantRevokeId = "probe_revoke_001"  // Identification.
        mandateId = "probe_mandate_001"  // Mandate Details.
        connectorMandateId = "probe_connector_mandate_001"
    }.build()
    val response = client.recurring_revoke(request)
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
        refundId = "probe_refund_id_001"  // Deprecated.
    }.build()
    val response = client.refund_get(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentService.Reverse
fun reverse(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = buildReverseRequest("probe_connector_txn_001")
    val response = client.reverse(request)
    println("Status: ${response.status.name}")
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

// Flow: PaymentService.TokenSetupRecurring
fun tokenSetupRecurring(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceTokenSetupRecurringRequest.newBuilder().apply {
        merchantRecurringPaymentId = "probe_tokenized_mandate_001"
        amountBuilder.apply {
            minorAmount = 0L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        connectorTokenBuilder.value = "pm_1AbcXyzStripeTestToken"
        addressBuilder.apply {
            billingAddressBuilder.apply {
            }
        }
        customerAcceptanceBuilder.apply {
            acceptanceType = AcceptanceType.ONLINE  // Type of acceptance (e.g., online, offline).
            acceptedAt = 0L  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
            onlineMandateDetailsBuilder.apply {  // Details if the acceptance was an online mandate.
                ipAddress = "127.0.0.1"  // IP address from which the mandate was accepted.
                userAgent = "Mozilla/5.0"  // User agent string of the browser used for mandate acceptance.
            }
        }
        setupMandateDetailsBuilder.apply {
            mandateTypeBuilder.apply {  // Type of mandate (single_use or multi_use) with amount details.
                multiUseBuilder.apply {  // Multi use mandate with amount details (for recurring payments).
                    amount = 0L  // Use amount_money instead (will be removed in a future release).
                    currency = Currency.USD  // Use amount_money.currency instead (will be removed in a future release).
                    amountMoneyBuilder.apply {  // Amount in Money type.
                        minorAmount = 0L  // Amount in minor units (e.g., 1000 = $10.00).
                        currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
                    }
                }
            }
        }
        setupFutureUsage = FutureUsage.OFF_SESSION
    }.build()
    val response = client.token_setup_recurring(request)
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
    val flow = args.firstOrNull() ?: "capture"
    when (flow) {
        "capture" -> capture(txnId)
        "createClientAuthenticationToken" -> createClientAuthenticationToken(txnId)
        "get" -> get(txnId)
        "handleEvent" -> handleEvent(txnId)
        "parseEvent" -> parseEvent(txnId)
        "postAuthenticate" -> postAuthenticate(txnId)
        "preAuthenticate" -> preAuthenticate(txnId)
        "recurringRevoke" -> recurringRevoke(txnId)
        "refund" -> refund(txnId)
        "refundGet" -> refundGet(txnId)
        "reverse" -> reverse(txnId)
        "tokenAuthorize" -> tokenAuthorize(txnId)
        "tokenSetupRecurring" -> tokenSetupRecurring(txnId)
        "void" -> void(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: capture, createClientAuthenticationToken, get, handleEvent, parseEvent, postAuthenticate, preAuthenticate, recurringRevoke, refund, refundGet, reverse, tokenAuthorize, tokenSetupRecurring, void")
    }
}
