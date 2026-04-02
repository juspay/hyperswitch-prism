// fiservcommercehub SDK Examples

package examples

import com.payments.sdk.*
import com.google.protobuf.util.JsonFormat

fun _buildrefundRequest(arg: String? = null): refundRequest {
    val json = """{"merchant_refund_id": "probe_refund_001", "connector_transaction_id": "probe_connector_txn_001", "payment_amount": 1000, "refund_amount": {"minor_amount": 1000, "currency": "USD"}, "reason": "customer_request", "state": {"access_token": {"token": "probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA", "expires_in_seconds": 3600, "token_type": "Bearer"}}}""".trimIndent()
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
    val json = """{"merchant_void_id": "probe_void_001", "connector_transaction_id": "probe_connector_txn_001", "state": {"access_token": {"token": "probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA", "expires_in_seconds": 3600, "token_type": "Bearer"}}}""".trimIndent()
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
    val json = """{"merchant_transaction_id": "probe_merchant_txn_001", "connector_transaction_id": "probe_connector_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}, "state": {"access_token": {"token": "probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA", "expires_in_seconds": 3600, "token_type": "Bearer"}}}""".trimIndent()
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
