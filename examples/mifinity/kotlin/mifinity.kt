// Auto-generated for mifinity
package examples.mifinity

import payments.ConnectorConfig
import payments.Currency
import payments.DirectPaymentClient
import payments.Environment
import payments.Money
import payments.PaymentServiceGetRequest

fun get(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.get
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.get(PaymentServiceGetRequest.newBuilder().setMerchantTransactionId("probe_merchant_txn_001").setConnectorTransactionId("probe_connector_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).build())
    println("[get] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}