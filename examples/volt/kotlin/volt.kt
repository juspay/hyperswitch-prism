// Auto-generated for volt
package examples.volt

import payments.AccessToken
import payments.ConnectorConfig
import payments.ConnectorState
import payments.Currency
import payments.DirectPaymentClient
import payments.Environment
import payments.MerchantAuthenticationClient
import payments.Money
import payments.PaymentServiceGetRequest
import payments.PaymentServiceRefundRequest
import payments.SecretString

fun create_access_token(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: MerchantAuthenticationService.create_access_token
    val merchantAuthenticationClient = MerchantAuthenticationClient(config)

    val result = merchantAuthenticationClient.create_access_token()
    println("[create_access_token] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun get(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.get
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.get(PaymentServiceGetRequest.newBuilder().setMerchantTransactionId("probe_merchant_txn_001").setConnectorTransactionId("probe_connector_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[get] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun refund(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.refund
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.refund(PaymentServiceRefundRequest.newBuilder().setMerchantRefundId("probe_refund_001").setConnectorTransactionId("probe_connector_txn_001").setPaymentAmount(1000).setRefundAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setReason("customer_request").setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[refund] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}