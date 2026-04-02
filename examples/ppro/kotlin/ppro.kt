// ppro SDK Examples

package examples

import com.payments.sdk.*
import com.google.protobuf.util.JsonFormat

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

fun _buildrecurringChargeRequest(arg: String? = null): recurringChargeRequest {
    val json = """{"connector_recurring_payment_id": {"mandate_id_type": {"connector_mandate_id": {"connector_mandate_id": "probe-mandate-123"}}}, "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"token": {"token": "probe_pm_token"}}, "return_url": "https://example.com/recurring-return", "connector_customer_id": "cust_probe_123", "payment_method_type": "PAY_PAL", "off_session": true}""".trimIndent()
    val builder = recurringChargeRequest.newBuilder()
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
