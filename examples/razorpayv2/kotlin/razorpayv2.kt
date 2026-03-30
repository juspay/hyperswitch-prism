// Auto-generated for razorpayv2
package examples.razorpayv2

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
import payments.PaymentServiceCreateOrderRequest
import payments.PaymentServiceGetRequest
import payments.PaymentServiceRefundRequest
import payments.SecretString
import payments.TokenizedPaymentClient
import types.Payment.TokenizedPaymentServiceAuthorizeRequest
import types.PaymentMethods.GooglePayEncryptedTokenizationData
import types.PaymentMethods.GoogleWallet
import types.PaymentMethods.Sepa

fun processCheckoutAutocapture(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Card Payment (Automatic Capture)
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setMerchantOrderId("probe_order_001").build())
    println("[authorize] HTTP ${result.statusCode}")

    return mapOf("statusCode" to result.statusCode)
}
fun processCheckoutWallet(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Wallet Payment (Google Pay / Apple Pay)
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setGooglePay(GoogleWallet.newBuilder().setType("CARD").setDescription("Visa 1111").setInfo(GoogleWallet.PaymentMethodInfo.newBuilder().setCardNetwork("VISA").setCardDetails("1111").build()).setTokenizationData(GoogleWallet.TokenizationData.newBuilder().setEncryptedData(GooglePayEncryptedTokenizationData.newBuilder().setTokenType("PAYMENT_GATEWAY").setToken("{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}").build()).build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setMerchantOrderId("probe_order_001").build())
    println("[authorize] HTTP ${result.statusCode}")

    return mapOf("statusCode" to result.statusCode)
}
fun processCheckoutBank(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Bank Transfer (SEPA / ACH / BACS)
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.EUR).build()).setPaymentMethod(PaymentMethod.newBuilder().setSepa(Sepa.newBuilder().setIban(SecretString.newBuilder().setValue("DE89370400440532013000").build()).setBankAccountHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setMerchantOrderId("probe_order_001").build())
    println("[authorize] HTTP ${result.statusCode}")

    return mapOf("statusCode" to result.statusCode)
}
fun processRefund(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Refund a Payment
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result1 = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setMerchantOrderId("probe_order_001").build())
    println("[authorize] HTTP ${result1.statusCode}")

    // Step 2: Refund — return funds to the customer
    val result2 = directPaymentClient.refund(PaymentServiceRefundRequest.newBuilder().setMerchantRefundId("probe_refund_001").setConnectorTransactionId("probe_connector_txn_001").setPaymentAmount(1000).setRefundAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setReason("customer_request").build())
    println("[refund] HTTP ${result2.statusCode}")

    return mapOf("statusCode" to result2.statusCode)
}
fun processGetPayment(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Get Payment Status
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result1 = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.MANUAL).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setMerchantOrderId("probe_order_001").build())
    println("[authorize] HTTP ${result1.statusCode}")

    // Step 2: Get — retrieve current payment status from the connector
    val result2 = directPaymentClient.get(PaymentServiceGetRequest.newBuilder().setMerchantTransactionId("probe_merchant_txn_001").setConnectorTransactionId("probe_connector_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).build())
    println("[get] HTTP ${result2.statusCode}")

    return mapOf("statusCode" to result2.statusCode)
}
fun authorize(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.authorize (Card)
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setMerchantOrderId("probe_order_001").build())
    println("[authorize] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun create_order(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.create_order
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.create_order(PaymentServiceCreateOrderRequest.newBuilder().setMerchantOrderId("probe_order_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).build())
    println("[create_order] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun get(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.get
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.get(PaymentServiceGetRequest.newBuilder().setMerchantTransactionId("probe_merchant_txn_001").setConnectorTransactionId("probe_connector_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).build())
    println("[get] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun refund(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.refund
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.refund(PaymentServiceRefundRequest.newBuilder().setMerchantRefundId("probe_refund_001").setConnectorTransactionId("probe_connector_txn_001").setPaymentAmount(1000).setRefundAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setReason("customer_request").build())
    println("[refund] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun tokenized_authorize(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: TokenizedPaymentService.tokenized_authorize
    val tokenizedPaymentClient = TokenizedPaymentClient(config)

    val result = tokenizedPaymentClient.tokenized_authorize(TokenizedPaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_tokenized_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setConnectorToken(SecretString.newBuilder().setValue("pm_1AbcXyzStripeTestToken").build()).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setReturnUrl("https://example.com/return").setMerchantOrderId("probe_order_001").build())
    println("[tokenized_authorize] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}