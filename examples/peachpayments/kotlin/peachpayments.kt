// peachpayments SDK Examples

package examples

import com.payments.sdk.*
import com.google.protobuf.util.JsonFormat

fun _buildauthorizeRequest(arg: String? = null): authorizeRequest {
    val json = """{"merchant_transaction_id": "probe_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}, "capture_method": "AUTOMATIC", "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"}""".trimIndent()
    val builder = authorizeRequest.newBuilder()
    JsonFormat.parser().merge(json, builder)
    if (arg != null) {
        when (arg) {
            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg
            else -> builder.connectorTransactionId = arg
        }
    }
    return builder.build()
}

fun _buildcaptureRequest(arg: String? = null): captureRequest {
    val json = """{"merchant_capture_id": "probe_capture_001", "connector_transaction_id": "probe_connector_txn_001", "amount_to_capture": {"minor_amount": 1000, "currency": "USD"}}""".trimIndent()
    val builder = captureRequest.newBuilder()
    JsonFormat.parser().merge(json, builder)
    if (arg != null) {
        when (arg) {
            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg
            else -> builder.connectorTransactionId = arg
        }
    }
    return builder.build()
}

fun _buildrefundRequest(arg: String? = null): refundRequest {
    val json = """{"merchant_refund_id": "probe_refund_001", "connector_transaction_id": "probe_connector_txn_001", "payment_amount": 1000, "refund_amount": {"minor_amount": 1000, "currency": "USD"}, "reason": "customer_request"}""".trimIndent()
    val builder = refundRequest.newBuilder()
    JsonFormat.parser().merge(json, builder)
    if (arg != null) {
        when (arg) {
            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg
            else -> builder.connectorTransactionId = arg
        }
    }
    return builder.build()
}

fun _buildvoidRequest(arg: String? = null): voidRequest {
    val json = """{"merchant_void_id": "probe_void_001", "connector_transaction_id": "probe_connector_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}}""".trimIndent()
    val builder = voidRequest.newBuilder()
    JsonFormat.parser().merge(json, builder)
    if (arg != null) {
        when (arg) {
            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg
            else -> builder.connectorTransactionId = arg
        }
    }
    return builder.build()
}

fun _buildgetRequest(arg: String? = null): getRequest {
    val json = """{"merchant_transaction_id": "probe_merchant_txn_001", "connector_transaction_id": "probe_connector_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}}""".trimIndent()
    val builder = getRequest.newBuilder()
    JsonFormat.parser().merge(json, builder)
    if (arg != null) {
        when (arg) {
            "AUTOMATIC", "MANUAL" -> builder.captureMethod = arg
            else -> builder.connectorTransactionId = arg
        }
    }
    return builder.build()
}
