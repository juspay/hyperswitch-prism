// Auto-generated for forte
package examples.forte

import payments.Address
import payments.AuthenticationType
import payments.CaptureMethod
import payments.CardDetails
import payments.CardNumberType
import payments.ConnectorConfig
import payments.Currency
import payments.DirectPaymentClient
import payments.Environment
import payments.Money
import payments.PaymentAddress
import payments.PaymentMethod
import payments.PaymentServiceAuthorizeRequest
import payments.PaymentServiceGetRequest
import payments.PaymentServiceVoidRequest
import payments.SecretString
import types.PaymentMethods.Ach

fun processCheckoutAutocapture(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Card Payment (Automatic Capture)
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().setFirstName(SecretString.newBuilder().setValue("John").build()).build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").build())
    println("[authorize] HTTP ${result.statusCode}")

    return mapOf("statusCode" to result.statusCode)
}
fun processCheckoutBank(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Bank Transfer (SEPA / ACH / BACS)
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setAch(Ach.newBuilder().setAccountNumber(SecretString.newBuilder().setValue("000123456789").build()).setRoutingNumber(SecretString.newBuilder().setValue("110000000").build()).setBankAccountHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().setFirstName(SecretString.newBuilder().setValue("John").build()).build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").build())
    println("[authorize] HTTP ${result.statusCode}")

    return mapOf("statusCode" to result.statusCode)
}
fun processVoidPayment(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Void a Payment
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result1 = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.MANUAL).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().setFirstName(SecretString.newBuilder().setValue("John").build()).build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").build())
    println("[authorize] HTTP ${result1.statusCode}")

    // Step 2: Void — release reserved funds (cancel authorization)
    val result2 = directPaymentClient.void(PaymentServiceVoidRequest.newBuilder().setMerchantVoidId("probe_void_001").setConnectorTransactionId("probe_connector_txn_001").build())
    println("[void] HTTP ${result2.statusCode}")

    return mapOf("statusCode" to result2.statusCode)
}
fun processGetPayment(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Get Payment Status
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result1 = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.MANUAL).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().setFirstName(SecretString.newBuilder().setValue("John").build()).build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").build())
    println("[authorize] HTTP ${result1.statusCode}")

    // Step 2: Get — retrieve current payment status from the connector
    val result2 = directPaymentClient.get(PaymentServiceGetRequest.newBuilder().setMerchantTransactionId("probe_merchant_txn_001").setConnectorTransactionId("probe_connector_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).build())
    println("[get] HTTP ${result2.statusCode}")

    return mapOf("statusCode" to result2.statusCode)
}
fun authorize(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.authorize (Card)
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().setFirstName(SecretString.newBuilder().setValue("John").build()).build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").build())
    println("[authorize] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun get(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.get
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.get(PaymentServiceGetRequest.newBuilder().setMerchantTransactionId("probe_merchant_txn_001").setConnectorTransactionId("probe_connector_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).build())
    println("[get] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun void(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.void
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.void(PaymentServiceVoidRequest.newBuilder().setMerchantVoidId("probe_void_001").setConnectorTransactionId("probe_connector_txn_001").build())
    println("[void] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}