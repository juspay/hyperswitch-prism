// Auto-generated for paytm
package examples.paytm

import payments.Address
import payments.AuthenticationType
import payments.CaptureMethod
import payments.ConnectorConfig
import payments.Currency
import payments.DirectPaymentClient
import payments.Environment
import payments.MerchantAuthenticationClient
import payments.MerchantAuthenticationServiceCreateSessionTokenRequest
import payments.Money
import payments.PaymentAddress
import payments.PaymentMethod
import payments.PaymentServiceAuthorizeRequest
import payments.PaymentServiceGetRequest
import payments.SecretString
import types.PaymentMethods.UpiCollect

fun authorize(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.authorize (UpiCollect)
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setUpiCollect(UpiCollect.newBuilder().setVpaId(SecretString.newBuilder().setValue("test@upi").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setSessionToken("probe_session_token").build())
    println("[authorize] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun create_session_token(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: MerchantAuthenticationService.create_session_token
    val merchantAuthenticationClient = MerchantAuthenticationClient(config)

    val result = merchantAuthenticationClient.create_session_token(MerchantAuthenticationServiceCreateSessionTokenRequest.newBuilder().setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).build())
    println("[create_session_token] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun get(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.get
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.get(PaymentServiceGetRequest.newBuilder().setMerchantTransactionId("probe_merchant_txn_001").setConnectorTransactionId("probe_connector_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).build())
    println("[get] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}