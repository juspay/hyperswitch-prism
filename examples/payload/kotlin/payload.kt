// Auto-generated for payload
package examples.payload

import payments.AccessToken
import payments.ConnectorConfig
import payments.ConnectorState
import payments.Currency
import payments.DirectPaymentClient
import payments.Environment
import payments.Money
import payments.PaymentMethod
import payments.PaymentMethodType
import payments.PaymentServiceCaptureRequest
import payments.PaymentServiceGetRequest
import payments.PaymentServiceRefundRequest
import payments.PaymentServiceVoidRequest
import payments.RecurringPaymentClient
import payments.RecurringPaymentServiceChargeRequest
import payments.SecretString
import payments.TokenPaymentMethodType
import types.Payment.ConnectorMandateReferenceId
import types.Payment.MandateReference

fun capture(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.capture
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.capture(PaymentServiceCaptureRequest.newBuilder().setMerchantCaptureId("probe_capture_001").setConnectorTransactionId("probe_connector_txn_001").setAmountToCapture(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[capture] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun get(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.get
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.get(PaymentServiceGetRequest.newBuilder().setMerchantTransactionId("probe_merchant_txn_001").setConnectorTransactionId("probe_connector_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[get] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun recurring_charge(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: RecurringPaymentService.recurring_charge
    val recurringPaymentClient = RecurringPaymentClient(config)

    val result = recurringPaymentClient.charge(RecurringPaymentServiceChargeRequest.newBuilder().setConnectorRecurringPaymentId(MandateReference.newBuilder().setConnectorMandateId(ConnectorMandateReferenceId.newBuilder().setConnectorMandateId("probe-mandate-123").build()).build()).setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setToken(TokenPaymentMethodType.newBuilder().setToken(SecretString.newBuilder().setValue("probe_pm_token").build()).build()).build()).setReturnUrl("https://example.com/recurring-return").setConnectorCustomerId("cust_probe_123").setPaymentMethodType(PaymentMethodType.PAY_PAL).setOffSession(true).setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[recurring_charge] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun refund(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.refund
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.refund(PaymentServiceRefundRequest.newBuilder().setMerchantRefundId("probe_refund_001").setConnectorTransactionId("probe_connector_txn_001").setPaymentAmount(1000).setRefundAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setReason("customer_request").setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[refund] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun void(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.void
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.void(PaymentServiceVoidRequest.newBuilder().setMerchantVoidId("probe_void_001").setConnectorTransactionId("probe_connector_txn_001").setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[void] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}