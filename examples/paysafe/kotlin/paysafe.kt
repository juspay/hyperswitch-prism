// Auto-generated for paysafe
package examples.paysafe

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
import payments.PaymentMethodClient
import payments.PaymentMethodServiceTokenizeRequest
import payments.PaymentServiceAuthorizeRequest
import payments.PaymentServiceCaptureRequest
import payments.PaymentServiceGetRequest
import payments.PaymentServiceRefundRequest
import payments.PaymentServiceVoidRequest
import payments.SecretString
import types.PaymentMethods.GooglePayEncryptedTokenizationData
import types.PaymentMethods.GoogleWallet
import types.PaymentMethods.Sepa

fun processCheckoutCard(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Card Payment (Authorize + Capture)
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result1 = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.MANUAL).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setPaymentMethodToken(SecretString.newBuilder().setValue("probe_pm_token").build()).build())
    println("[authorize] HTTP ${result1.statusCode}")

    // Step 2: Capture — settle the reserved funds
    val result2 = directPaymentClient.capture(PaymentServiceCaptureRequest.newBuilder().setMerchantCaptureId("probe_capture_001").setConnectorTransactionId("probe_connector_txn_001").setAmountToCapture(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).build())
    println("[capture] HTTP ${result2.statusCode}")

    return mapOf("statusCode" to result2.statusCode)
}
fun processCheckoutAutocapture(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Card Payment (Automatic Capture)
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setPaymentMethodToken(SecretString.newBuilder().setValue("probe_pm_token").build()).build())
    println("[authorize] HTTP ${result.statusCode}")

    return mapOf("statusCode" to result.statusCode)
}
fun processCheckoutWallet(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Wallet Payment (Google Pay / Apple Pay)
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setGooglePay(GoogleWallet.newBuilder().setType("CARD").setDescription("Visa 1111").setInfo(GoogleWallet.PaymentMethodInfo.newBuilder().setCardNetwork("VISA").setCardDetails("1111").build()).setTokenizationData(GoogleWallet.TokenizationData.newBuilder().setEncryptedData(GooglePayEncryptedTokenizationData.newBuilder().setTokenType("PAYMENT_GATEWAY").setToken("{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}").build()).build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setPaymentMethodToken(SecretString.newBuilder().setValue("probe_pm_token").build()).build())
    println("[authorize] HTTP ${result.statusCode}")

    return mapOf("statusCode" to result.statusCode)
}
fun processCheckoutBank(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Bank Transfer (SEPA / ACH / BACS)
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.EUR).build()).setPaymentMethod(PaymentMethod.newBuilder().setSepa(Sepa.newBuilder().setIban(SecretString.newBuilder().setValue("DE89370400440532013000").build()).setBankAccountHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setPaymentMethodToken(SecretString.newBuilder().setValue("probe_pm_token").build()).build())
    println("[authorize] HTTP ${result.statusCode}")

    return mapOf("statusCode" to result.statusCode)
}
fun processRefund(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Refund a Payment
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result1 = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setPaymentMethodToken(SecretString.newBuilder().setValue("probe_pm_token").build()).build())
    println("[authorize] HTTP ${result1.statusCode}")

    // Step 2: Refund — return funds to the customer
    val result2 = directPaymentClient.refund(PaymentServiceRefundRequest.newBuilder().setMerchantRefundId("probe_refund_001").setConnectorTransactionId("probe_connector_txn_001").setPaymentAmount(1000).setRefundAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setReason("customer_request").build())
    println("[refund] HTTP ${result2.statusCode}")

    return mapOf("statusCode" to result2.statusCode)
}
fun processVoidPayment(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Void a Payment
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result1 = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.MANUAL).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setPaymentMethodToken(SecretString.newBuilder().setValue("probe_pm_token").build()).build())
    println("[authorize] HTTP ${result1.statusCode}")

    // Step 2: Void — release reserved funds (cancel authorization)
    val result2 = directPaymentClient.void(PaymentServiceVoidRequest.newBuilder().setMerchantVoidId("probe_void_001").setConnectorTransactionId("probe_connector_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).build())
    println("[void] HTTP ${result2.statusCode}")

    return mapOf("statusCode" to result2.statusCode)
}
fun processGetPayment(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Get Payment Status
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result1 = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.MANUAL).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setPaymentMethodToken(SecretString.newBuilder().setValue("probe_pm_token").build()).build())
    println("[authorize] HTTP ${result1.statusCode}")

    // Step 2: Get — retrieve current payment status from the connector
    val result2 = directPaymentClient.get(PaymentServiceGetRequest.newBuilder().setMerchantTransactionId("probe_merchant_txn_001").setConnectorTransactionId("probe_connector_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).build())
    println("[get] HTTP ${result2.statusCode}")

    return mapOf("statusCode" to result2.statusCode)
}
fun processTokenize(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Tokenize Payment Method
    val paymentMethodClient = PaymentMethodClient(config)

    // Step 1: Tokenize — store card details and return a reusable token
    val result = paymentMethodClient.tokenize(PaymentMethodServiceTokenizeRequest.newBuilder().setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setReturnUrl("https://example.com/return").build())
    println("[tokenize] HTTP ${result.statusCode}")

    return mapOf("statusCode" to result.statusCode)
}
fun authorize(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.authorize (Card)
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setPaymentMethodToken(SecretString.newBuilder().setValue("probe_pm_token").build()).build())
    println("[authorize] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun capture(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.capture
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.capture(PaymentServiceCaptureRequest.newBuilder().setMerchantCaptureId("probe_capture_001").setConnectorTransactionId("probe_connector_txn_001").setAmountToCapture(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).build())
    println("[capture] HTTP ${result.statusCode}")
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
fun tokenize(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentMethodService.tokenize
    val paymentMethodClient = PaymentMethodClient(config)

    val result = paymentMethodClient.tokenize(PaymentMethodServiceTokenizeRequest.newBuilder().setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setReturnUrl("https://example.com/return").build())
    println("[tokenize] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun void(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.void
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.void(PaymentServiceVoidRequest.newBuilder().setMerchantVoidId("probe_void_001").setConnectorTransactionId("probe_connector_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).build())
    println("[void] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}