// Auto-generated for trustpay
package examples.trustpay

import payments.AccessToken
import payments.Address
import payments.AuthenticationType
import payments.BrowserInformation
import payments.CaptureMethod
import payments.CardDetails
import payments.CardNumberType
import payments.ConnectorConfig
import payments.ConnectorState
import payments.CountryAlpha2
import payments.Currency
import payments.Customer
import payments.DirectPaymentClient
import payments.Environment
import payments.MerchantAuthenticationClient
import payments.Money
import payments.PaymentAddress
import payments.PaymentMethod
import payments.PaymentServiceAuthorizeRequest
import payments.PaymentServiceCreateOrderRequest
import payments.PaymentServiceGetRequest
import payments.PaymentServiceRefundRequest
import payments.SecretString

fun processCheckoutAutocapture(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Card Payment (Automatic Capture)
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setCustomer(Customer.newBuilder().setEmail(SecretString.newBuilder().setValue("test@example.com").build()).build()).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().setFirstName(SecretString.newBuilder().setValue("John").build()).setLine1(SecretString.newBuilder().setValue("123 Main St").build()).setCity(SecretString.newBuilder().setValue("Seattle").build()).setZipCode(SecretString.newBuilder().setValue("98101").build()).setCountryAlpha2Code(CountryAlpha2.US).build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setBrowserInfo(BrowserInformation.newBuilder().setUserAgent("Mozilla/5.0 (probe-bot)").setIpAddress("1.2.3.4").build()).setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[authorize] HTTP ${result.statusCode}")

    return mapOf("statusCode" to result.statusCode)
}
fun processRefund(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Refund a Payment
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result1 = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setCustomer(Customer.newBuilder().setEmail(SecretString.newBuilder().setValue("test@example.com").build()).build()).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().setFirstName(SecretString.newBuilder().setValue("John").build()).setLine1(SecretString.newBuilder().setValue("123 Main St").build()).setCity(SecretString.newBuilder().setValue("Seattle").build()).setZipCode(SecretString.newBuilder().setValue("98101").build()).setCountryAlpha2Code(CountryAlpha2.US).build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setBrowserInfo(BrowserInformation.newBuilder().setUserAgent("Mozilla/5.0 (probe-bot)").setIpAddress("1.2.3.4").build()).setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[authorize] HTTP ${result1.statusCode}")

    // Step 2: Refund — return funds to the customer
    val result2 = directPaymentClient.refund(PaymentServiceRefundRequest.newBuilder().setMerchantRefundId("probe_refund_001").setConnectorTransactionId("probe_connector_txn_001").setPaymentAmount(1000).setRefundAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setReason("customer_request").setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[refund] HTTP ${result2.statusCode}")

    return mapOf("statusCode" to result2.statusCode)
}
fun processGetPayment(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Get Payment Status
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result1 = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.MANUAL).setCustomer(Customer.newBuilder().setEmail(SecretString.newBuilder().setValue("test@example.com").build()).build()).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().setFirstName(SecretString.newBuilder().setValue("John").build()).setLine1(SecretString.newBuilder().setValue("123 Main St").build()).setCity(SecretString.newBuilder().setValue("Seattle").build()).setZipCode(SecretString.newBuilder().setValue("98101").build()).setCountryAlpha2Code(CountryAlpha2.US).build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setBrowserInfo(BrowserInformation.newBuilder().setUserAgent("Mozilla/5.0 (probe-bot)").setIpAddress("1.2.3.4").build()).setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[authorize] HTTP ${result1.statusCode}")

    // Step 2: Get — retrieve current payment status from the connector
    val result2 = directPaymentClient.get(PaymentServiceGetRequest.newBuilder().setMerchantTransactionId("probe_merchant_txn_001").setConnectorTransactionId("probe_connector_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[get] HTTP ${result2.statusCode}")

    return mapOf("statusCode" to result2.statusCode)
}
fun authorize(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.authorize (Card)
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setCustomer(Customer.newBuilder().setEmail(SecretString.newBuilder().setValue("test@example.com").build()).build()).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().setFirstName(SecretString.newBuilder().setValue("John").build()).setLine1(SecretString.newBuilder().setValue("123 Main St").build()).setCity(SecretString.newBuilder().setValue("Seattle").build()).setZipCode(SecretString.newBuilder().setValue("98101").build()).setCountryAlpha2Code(CountryAlpha2.US).build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setBrowserInfo(BrowserInformation.newBuilder().setUserAgent("Mozilla/5.0 (probe-bot)").setIpAddress("1.2.3.4").build()).setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[authorize] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun create_access_token(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: MerchantAuthenticationService.create_access_token
    val merchantAuthenticationClient = MerchantAuthenticationClient(config)

    val result = merchantAuthenticationClient.create_access_token()
    println("[create_access_token] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun create_order(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.create_order
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.create_order(PaymentServiceCreateOrderRequest.newBuilder().setMerchantOrderId("probe_order_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setState(ConnectorState.newBuilder().setAccessToken(AccessToken.newBuilder().setToken(SecretString.newBuilder().setValue("probe_access_token").build()).setExpiresInSeconds(3600).setTokenType("Bearer").build()).build()).build())
    println("[create_order] HTTP ${result.statusCode}")
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