// stripe SDK Examples

package examples

import com.payments.sdk.*
import com.google.protobuf.util.JsonFormat

fun _buildAuthorizeRequest(arg: String? = null): PaymentServiceAuthorizeRequest {
    val json = """{"merchant_transaction_id": "probe_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}, "capture_method": "AUTOMATIC", "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"}""".trimIndent()
    val builder = PaymentServiceAuthorizeRequest.newBuilder()
    JsonFormat.parser().merge(json, builder)
    if (arg != null) {
        when (arg) {
            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg
            else -> builder.connectorTransactionId = arg
        }
    }
    return builder.build()
}

fun _buildCaptureRequest(arg: String? = null): PaymentServiceCaptureRequest {
    val json = """{"merchant_capture_id": "probe_capture_001", "connector_transaction_id": "probe_connector_txn_001", "amount_to_capture": {"minor_amount": 1000, "currency": "USD"}}""".trimIndent()
    val builder = PaymentServiceCaptureRequest.newBuilder()
    JsonFormat.parser().merge(json, builder)
    if (arg != null) {
        when (arg) {
            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg
            else -> builder.connectorTransactionId = arg
        }
    }
    return builder.build()
}

fun _buildRefundRequest(arg: String? = null): PaymentServiceRefundRequest {
    val json = """{"merchant_refund_id": "probe_refund_001", "connector_transaction_id": "probe_connector_txn_001", "payment_amount": 1000, "refund_amount": {"minor_amount": 1000, "currency": "USD"}, "reason": "customer_request"}""".trimIndent()
    val builder = PaymentServiceRefundRequest.newBuilder()
    JsonFormat.parser().merge(json, builder)
    if (arg != null) {
        when (arg) {
            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg
            else -> builder.connectorTransactionId = arg
        }
    }
    return builder.build()
}

fun _buildSetupRecurringRequest(arg: String? = null): PaymentServiceSetupRecurringRequest {
    val json = """{"merchant_recurring_payment_id": "probe_mandate_001", "amount": {"minor_amount": 0, "currency": "USD"}, "payment_method": {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}, "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "enrolled_for_3ds": false, "return_url": "https://example.com/mandate-return", "setup_future_usage": "OFF_SESSION", "request_incremental_authorization": false, "customer_acceptance": {"acceptance_type": "OFFLINE", "accepted_at": 0}}""".trimIndent()
    val builder = PaymentServiceSetupRecurringRequest.newBuilder()
    JsonFormat.parser().merge(json, builder)
    if (arg != null) {
        when (arg) {
            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg
            else -> builder.connectorTransactionId = arg
        }
    }
    return builder.build()
}

fun _buildRecurringChargeRequest(arg: String? = null): RecurringPaymentServiceChargeRequest {
    val json = """{"connector_recurring_payment_id": {"mandate_id_type": {"connector_mandate_id": {"connector_mandate_id": "probe-mandate-123"}}}, "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"token": {"token": "probe_pm_token"}}, "return_url": "https://example.com/recurring-return", "connector_customer_id": "cust_probe_123", "payment_method_type": "PAY_PAL", "off_session": true}""".trimIndent()
    val builder = RecurringPaymentServiceChargeRequest.newBuilder()
    JsonFormat.parser().merge(json, builder)
    if (arg != null) {
        when (arg) {
            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg
            else -> builder.connectorTransactionId = arg
        }
    }
    return builder.build()
}

fun _buildTokenizeRequest(arg: String? = null): PaymentMethodServiceTokenizeRequest {
    val json = """{"amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}, "address": {"billing_address": {}}}""".trimIndent()
    val builder = PaymentMethodServiceTokenizeRequest.newBuilder()
    JsonFormat.parser().merge(json, builder)
    if (arg != null) {
        when (arg) {
            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg
            else -> builder.connectorTransactionId = arg
        }
    }
    return builder.build()
}

fun _buildVoidRequest(arg: String? = null): PaymentServiceVoidRequest {
    val json = """{"merchant_void_id": "probe_void_001", "connector_transaction_id": "probe_connector_txn_001"}""".trimIndent()
    val builder = PaymentServiceVoidRequest.newBuilder()
    JsonFormat.parser().merge(json, builder)
    if (arg != null) {
        when (arg) {
            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg
            else -> builder.connectorTransactionId = arg
        }
    }
    return builder.build()
}

fun _buildGetRequest(arg: String? = null): PaymentServiceGetRequest {
    val json = """{"merchant_transaction_id": "probe_merchant_txn_001", "connector_transaction_id": "probe_connector_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}}""".trimIndent()
    val builder = PaymentServiceGetRequest.newBuilder()
    JsonFormat.parser().merge(json, builder)
    if (arg != null) {
        when (arg) {
            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg
            else -> builder.connectorTransactionId = arg
        }
    }
    return builder.build()
}

suspend fun processCheckoutCard() {
    // Standard card authorization and capture flow
    val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"
        amount = minorAmount.newBuilder().build()
        paymentMethod = card.newBuilder().build()
        captureMethod = "AUTOMATIC"
        address = billingAddress.newBuilder().build()
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
    }.build()
    val authorizeResponse = client.authorize(request)
    when (authorizeResponse.status) {
        "FAILED", "AUTHORIZATION_FAILED" -> throw RuntimeException("Payment authorization failed: $error")
        else -> {}
    }
    when (authorizeResponse.status) {
        "PENDING" -> return mapOf("status" to authorizeResponse.status, "transaction_id" to authorizeResponse.connectorTransactionId)
        else -> {}
    }

    val request = PaymentServiceCaptureRequest.newBuilder().apply {
        merchantCaptureId = "probe_capture_001"
        connectorTransactionId = authorizeResponse.connectorTransactionId
        amountToCapture = minorAmount.newBuilder().build()
    }.build()
    val captureResponse = client.capture(request)
    when (captureResponse.status) {
        "FAILED" -> throw RuntimeException("Capture failed: $error")
        else -> {}
    }

    return mapOf(
        "status" to captureResponse.status,
        "transaction_id" to captureResponse.connectorTransactionId,
        "amount" to captureResponse.amount,
    )
}

suspend fun processCheckoutBank() {
    // Bank transfer or debit payment flow
    val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"
        amount = minorAmount.newBuilder().build()
        paymentMethod = ach.newBuilder().build()
        captureMethod = "AUTOMATIC"
        address = billingAddress.newBuilder().build()
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
    }.build()
    val authorizeResponse = client.authorize(request)
    when (authorizeResponse.status) {
        "FAILED" -> throw RuntimeException("Bank transfer failed: $error")
        else -> {}
    }

    return mapOf(
        "status" to authorizeResponse.status,
        "transaction_id" to authorizeResponse.connectorTransactionId,
    )
}

suspend fun processCheckoutWallet() {
    // Apple Pay, Google Pay, or other wallet payment
    val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"
        amount = minorAmount.newBuilder().build()
        paymentMethod = applePay.newBuilder().build()
        captureMethod = "AUTOMATIC"
        address = billingAddress.newBuilder().build()
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
        paymentMethodToken = "probe_pm_token"
    }.build()
    val authorizeResponse = client.authorize(request)
    when (authorizeResponse.status) {
        "FAILED", "AUTHORIZATION_FAILED" -> throw RuntimeException("Wallet payment failed: $error")
        else -> {}
    }

    val request = PaymentServiceCaptureRequest.newBuilder().apply {
        merchantCaptureId = "probe_capture_001"
        connectorTransactionId = authorizeResponse.connectorTransactionId
        amountToCapture = minorAmount.newBuilder().build()
    }.build()
    val captureResponse = client.capture(request)

    return mapOf(
        "status" to captureResponse.status,
        "transaction_id" to captureResponse.connectorTransactionId,
    )
}

suspend fun processRefundPayment() {
    // Refund a completed payment
    val request = PaymentServiceRefundRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"
        connectorTransactionId = authorize_response.connectorTransactionId
        paymentAmount = 1000
        refundAmount = minorAmount.newBuilder().build()
        reason = "customer_request"
    }.build()
    val refundResponse = client.refund(request)
    when (refundResponse.status) {
        "FAILED" -> throw RuntimeException("Refund failed: $error")
        else -> {}
    }

    return mapOf(
        "status" to refundResponse.status,
        "refund_id" to refundResponse.connectorRefundId,
    )
}

suspend fun processSetupRecurring() {
    // Create a mandate for recurring charges
    val request = PaymentServiceSetupRecurringRequest.newBuilder().apply {
        merchantRecurringPaymentId = "probe_mandate_001"
        amount = minorAmount.newBuilder().build()
        paymentMethod = card.newBuilder().build()
        address = billingAddress.newBuilder().build()
        authType = "NO_THREE_DS"
        enrolledFor3ds = false
        returnUrl = "https://example.com/mandate-return"
        setupFutureUsage = "OFF_SESSION"
        requestIncrementalAuthorization = false
        customerAcceptance = acceptanceType.newBuilder().build()
    }.build()
    val setupRecurringResponse = client.setup_recurring(request)
    when (setupRecurringResponse.status) {
        "FAILED" -> throw RuntimeException("Failed to setup recurring payment: $error")
        else -> {}
    }

    return mapOf(
        "status" to setupRecurringResponse.status,
        "mandate_id" to setupRecurringResponse.mandateReference.connectorMandateId,
    )
}

suspend fun processRecurringCharge() {
    // Charge against an existing mandate
    // Prerequisite: Create customer profile
    val request = CustomerServiceCreateRequest.newBuilder().apply {
        merchantCustomerId = "cust_probe_123"
        customerName = "John Doe"
        email = "test@example.com"
        phoneNumber = "4155552671"
    }.build()
    val createCustomerResponse = client.create_customer(request)

    val request = RecurringPaymentServiceChargeRequest.newBuilder().apply {
        connectorRecurringPaymentId = mandateIdType.newBuilder().build()
        amount = minorAmount.newBuilder().build()
        paymentMethod = token.newBuilder().build()
        returnUrl = "https://example.com/recurring-return"
        connectorCustomerId = create_customer_response.connectorCustomerId
        paymentMethodType = "PAY_PAL"
        offSession = true
    }.build()
    val recurringChargeResponse = client.recurring_charge(request)
    when (recurringChargeResponse.status) {
        "FAILED" -> throw RuntimeException("Recurring charge failed: $error")
        else -> {}
    }

    return mapOf(
        "status" to recurringChargeResponse.status,
        "transaction_id" to recurringChargeResponse.connectorTransactionId,
    )
}

suspend fun processTokenizePaymentMethod() {
    // Tokenize a card or bank account for later use
    val request = PaymentMethodServiceTokenizeRequest.newBuilder().apply {
        amount = minorAmount.newBuilder().build()
        paymentMethod = card.newBuilder().build()
        address = billingAddress.newBuilder().build()
    }.build()
    val tokenizeResponse = client.tokenize(request)
    when (tokenizeResponse.status) {
        "FAILED" -> throw RuntimeException("Tokenization failed: $error")
        else -> {}
    }

    return mapOf(
        "status" to tokenizeResponse.status,
        "token" to tokenizeResponse.paymentMethodToken,
    )
}

suspend fun processVoidAuthorization() {
    // Cancel an uncaptured authorization
    val request = PaymentServiceVoidRequest.newBuilder().apply {
        merchantVoidId = "probe_void_001"
        connectorTransactionId = authorize_response.connectorTransactionId
    }.build()
    val voidResponse = client.void(request)
    when (voidResponse.status) {
        "FAILED" -> throw RuntimeException("Void failed: $error")
        else -> {}
    }

    return mapOf(
        "status" to voidResponse.status,
    )
}

suspend fun processGetPaymentStatus() {
    // Retrieve current status of a payment
    val request = PaymentServiceGetRequest.newBuilder().apply {
        merchantTransactionId = "probe_merchant_txn_001"
        connectorTransactionId = authorize_response.connectorTransactionId
        amount = minorAmount.newBuilder().build()
    }.build()
    val getResponse = client.get(request)

    return mapOf(
        "status" to getResponse.status,
        "amount" to getResponse.amount,
    )
}

suspend fun processPartialRefund() {
    // Refund a portion of a captured payment
    val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"
        amount = minorAmount.newBuilder().build()
        paymentMethod = card.newBuilder().build()
        captureMethod = "AUTOMATIC"
        address = billingAddress.newBuilder().build()
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
    }.build()
    val authorizeResponse = client.authorize(request)
    when (authorizeResponse.status) {
        "FAILED", "AUTHORIZATION_FAILED" -> throw RuntimeException("Payment authorization failed: $error")
        else -> {}
    }

    val request = PaymentServiceCaptureRequest.newBuilder().apply {
        merchantCaptureId = "probe_capture_001"
        connectorTransactionId = authorizeResponse.connectorTransactionId
        amountToCapture = minorAmount.newBuilder().build()
    }.build()
    val captureResponse = client.capture(request)
    when (captureResponse.status) {
        "FAILED" -> throw RuntimeException("Capture failed: $error")
        else -> {}
    }

    val request = PaymentServiceRefundRequest.newBuilder().apply {
        merchantRefundId = "probe_refund_001"
        connectorTransactionId = authorizeResponse.connectorTransactionId
        paymentAmount = 1000
        refundAmount = minorAmount.newBuilder().build()
        reason = "customer_request"
    }.build()
    val refundResponse = client.refund(request)
    when (refundResponse.status) {
        "FAILED" -> throw RuntimeException("Refund failed: $error")
        else -> {}
    }

    return mapOf(
        "status" to refundResponse.status,
        "refund_id" to refundResponse.connectorRefundId,
        "refunded_amount" to refundResponse.amount,
    )
}

suspend fun processMultiCapture() {
    // Split a single authorization into multiple captures (e.g., for split shipments)
    val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"
        amount = minorAmount.newBuilder().build()
        paymentMethod = card.newBuilder().build()
        captureMethod = "AUTOMATIC"
        address = billingAddress.newBuilder().build()
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
    }.build()
    val authorizeResponse = client.authorize(request)
    when (authorizeResponse.status) {
        "FAILED", "AUTHORIZATION_FAILED" -> throw RuntimeException("Payment authorization failed: $error")
        else -> {}
    }

    val request = PaymentServiceCaptureRequest.newBuilder().apply {
        merchantCaptureId = "probe_capture_001"
        connectorTransactionId = authorizeResponse.connectorTransactionId
        amountToCapture = minorAmount.newBuilder().build()
    }.build()
    val captureResponse = client.capture(request)
    when (captureResponse.status) {
        "FAILED" -> throw RuntimeException("Capture failed: $error")
        else -> {}
    }

    return mapOf(
        "status" to captureResponse.status,
        "transaction_id" to captureResponse.connectorTransactionId,
        "captured_amount" to captureResponse.amount,
    )
}

suspend fun processIncrementalAuthorization() {
    // Increase the authorized amount after initial authorization
    val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"
        amount = minorAmount.newBuilder().build()
        paymentMethod = card.newBuilder().build()
        captureMethod = "AUTOMATIC"
        address = billingAddress.newBuilder().build()
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
    }.build()
    val authorizeResponse = client.authorize(request)
    when (authorizeResponse.status) {
        "FAILED", "AUTHORIZATION_FAILED" -> throw RuntimeException("Payment authorization failed: $error")
        else -> {}
    }

    val request = PaymentServiceCaptureRequest.newBuilder().apply {
        merchantCaptureId = "probe_capture_001"
        connectorTransactionId = authorizeResponse.connectorTransactionId
        amountToCapture = minorAmount.newBuilder().build()
    }.build()
    val captureResponse = client.capture(request)

    return mapOf(
        "status" to captureResponse.status,
        "transaction_id" to captureResponse.connectorTransactionId,
        "authorized_amount" to captureResponse.amount,
    )
}

suspend fun processCheckout3ds() {
    // Card payment with 3D Secure authentication
    val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"
        amount = minorAmount.newBuilder().build()
        paymentMethod = card.newBuilder().build()
        captureMethod = "AUTOMATIC"
        address = billingAddress.newBuilder().build()
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
    }.build()
    val authorizeResponse = client.authorize(request)
    when (authorizeResponse.status) {
        "FAILED", "AUTHORIZATION_FAILED" -> throw RuntimeException("Payment authorization failed: $error")
        else -> {}
    }
    when (authorizeResponse.status) {
        "PENDING_AUTHENTICATION" -> return mapOf("status" to authorizeResponse.status, "transaction_id" to authorizeResponse.connectorTransactionId, "redirect_url" to authorizeResponse.nextAction.redirectUrl)
        else -> {}
    }

    val request = PaymentServiceCaptureRequest.newBuilder().apply {
        merchantCaptureId = "probe_capture_001"
        connectorTransactionId = authorizeResponse.connectorTransactionId
        amountToCapture = minorAmount.newBuilder().build()
    }.build()
    val captureResponse = client.capture(request)

    return mapOf(
        "status" to captureResponse.status,
        "transaction_id" to captureResponse.connectorTransactionId,
    )
}

suspend fun processCheckoutBnpl() {
    // Buy Now Pay Later payment flow (Klarna, Afterpay, Affirm)
    val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_txn_001"
        amount = minorAmount.newBuilder().build()
        paymentMethod = klarna.newBuilder().build()
        captureMethod = "AUTOMATIC"
        address = billingAddress.newBuilder().build()
        authType = "NO_THREE_DS"
        returnUrl = "https://example.com/return"
    }.build()
    val authorizeResponse = client.authorize(request)
    when (authorizeResponse.status) {
        "FAILED", "AUTHORIZATION_FAILED" -> throw RuntimeException("BNPL authorization failed: $error")
        else -> {}
    }
    when (authorizeResponse.status) {
        "PENDING_AUTHENTICATION" -> return mapOf("status" to authorizeResponse.status, "transaction_id" to authorizeResponse.connectorTransactionId, "redirect_url" to authorizeResponse.nextAction.redirectUrl)
        else -> {}
    }

    val request = PaymentServiceCaptureRequest.newBuilder().apply {
        merchantCaptureId = "probe_capture_001"
        connectorTransactionId = authorizeResponse.connectorTransactionId
        amountToCapture = minorAmount.newBuilder().build()
    }.build()
    val captureResponse = client.capture(request)
    when (captureResponse.status) {
        "FAILED" -> throw RuntimeException("BNPL capture failed: $error")
        else -> {}
    }

    return mapOf(
        "status" to captureResponse.status,
        "transaction_id" to captureResponse.connectorTransactionId,
    )
}
