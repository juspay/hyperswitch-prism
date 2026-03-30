// Auto-generated for adyen
package examples.adyen

import payments.AcceptanceType
import payments.Address
import payments.AuthenticationType
import payments.BrowserInformation
import payments.CaptureMethod
import payments.CardDetails
import payments.CardNumberType
import payments.ConnectorConfig
import payments.Currency
import payments.Customer
import payments.CustomerAcceptance
import payments.DirectPaymentClient
import payments.DisputeClient
import payments.DisputeServiceAcceptRequest
import payments.DisputeServiceDefendRequest
import payments.DisputeServiceSubmitEvidenceRequest
import payments.Environment
import payments.FutureUsage
import payments.Money
import payments.PaymentAddress
import payments.PaymentMethod
import payments.PaymentMethodType
import payments.PaymentServiceAuthorizeRequest
import payments.PaymentServiceCaptureRequest
import payments.PaymentServiceRefundRequest
import payments.PaymentServiceSetupRecurringRequest
import payments.PaymentServiceVoidRequest
import payments.RecurringPaymentClient
import payments.RecurringPaymentServiceChargeRequest
import payments.SecretString
import payments.TokenPaymentMethodType
import types.Payment.ConnectorMandateReferenceId
import types.Payment.MandateReference
import types.PaymentMethods.GooglePayEncryptedTokenizationData
import types.PaymentMethods.GoogleWallet
import types.PaymentMethods.Sepa

fun processCheckoutCard(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Card Payment (Authorize + Capture)
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result1 = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.MANUAL).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setBrowserInfo(BrowserInformation.newBuilder().setColorDepth(24).setScreenHeight(900).setScreenWidth(1440).setJavaEnabled(false).setJavaScriptEnabled(true).setLanguage("en-US").setTimeZoneOffsetMinutes(-480).setAcceptHeader("application/json").setUserAgent("Mozilla/5.0 (probe-bot)").setAcceptLanguage("en-US,en;q=0.9").setIpAddress("1.2.3.4").build()).build())
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
    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setBrowserInfo(BrowserInformation.newBuilder().setColorDepth(24).setScreenHeight(900).setScreenWidth(1440).setJavaEnabled(false).setJavaScriptEnabled(true).setLanguage("en-US").setTimeZoneOffsetMinutes(-480).setAcceptHeader("application/json").setUserAgent("Mozilla/5.0 (probe-bot)").setAcceptLanguage("en-US,en;q=0.9").setIpAddress("1.2.3.4").build()).build())
    println("[authorize] HTTP ${result.statusCode}")

    return mapOf("statusCode" to result.statusCode)
}
fun processCheckoutWallet(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Wallet Payment (Google Pay / Apple Pay)
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setGooglePay(GoogleWallet.newBuilder().setType("CARD").setDescription("Visa 1111").setInfo(GoogleWallet.PaymentMethodInfo.newBuilder().setCardNetwork("VISA").setCardDetails("1111").build()).setTokenizationData(GoogleWallet.TokenizationData.newBuilder().setEncryptedData(GooglePayEncryptedTokenizationData.newBuilder().setTokenType("PAYMENT_GATEWAY").setToken("{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}").build()).build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setBrowserInfo(BrowserInformation.newBuilder().setColorDepth(24).setScreenHeight(900).setScreenWidth(1440).setJavaEnabled(false).setJavaScriptEnabled(true).setLanguage("en-US").setTimeZoneOffsetMinutes(-480).setAcceptHeader("application/json").setUserAgent("Mozilla/5.0 (probe-bot)").setAcceptLanguage("en-US,en;q=0.9").setIpAddress("1.2.3.4").build()).build())
    println("[authorize] HTTP ${result.statusCode}")

    return mapOf("statusCode" to result.statusCode)
}
fun processCheckoutBank(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Bank Transfer (SEPA / ACH / BACS)
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.EUR).build()).setPaymentMethod(PaymentMethod.newBuilder().setSepa(Sepa.newBuilder().setIban(SecretString.newBuilder().setValue("DE89370400440532013000").build()).setBankAccountHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().setFirstName(SecretString.newBuilder().setValue("John").build()).build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").build())
    println("[authorize] HTTP ${result.statusCode}")

    return mapOf("statusCode" to result.statusCode)
}
fun processRefund(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Refund a Payment
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result1 = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setBrowserInfo(BrowserInformation.newBuilder().setColorDepth(24).setScreenHeight(900).setScreenWidth(1440).setJavaEnabled(false).setJavaScriptEnabled(true).setLanguage("en-US").setTimeZoneOffsetMinutes(-480).setAcceptHeader("application/json").setUserAgent("Mozilla/5.0 (probe-bot)").setAcceptLanguage("en-US,en;q=0.9").setIpAddress("1.2.3.4").build()).build())
    println("[authorize] HTTP ${result1.statusCode}")

    // Step 2: Refund — return funds to the customer
    val result2 = directPaymentClient.refund(PaymentServiceRefundRequest.newBuilder().setMerchantRefundId("probe_refund_001").setConnectorTransactionId("probe_connector_txn_001").setPaymentAmount(1000).setRefundAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setReason("customer_request").build())
    println("[refund] HTTP ${result2.statusCode}")

    return mapOf("statusCode" to result2.statusCode)
}
fun processRecurring(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Recurring / Mandate Payments
    val directPaymentClient = DirectPaymentClient(config)
    val recurringPaymentClient = RecurringPaymentClient(config)

    // Step 1: Setup Recurring — store the payment mandate
    val result1 = directPaymentClient.setup_recurring(PaymentServiceSetupRecurringRequest.newBuilder().setMerchantRecurringPaymentId("probe_mandate_001").setAmount(Money.newBuilder().setMinorAmount(0).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCustomer(Customer.newBuilder().setId("cust_probe_123").build()).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setEnrolledFor3Ds(false).setReturnUrl("https://example.com/mandate-return").setSetupFutureUsage(FutureUsage.OFF_SESSION).setRequestIncrementalAuthorization(false).setCustomerAcceptance(CustomerAcceptance.newBuilder().setAcceptanceType(AcceptanceType.OFFLINE).setAcceptedAt(0).build()).setBrowserInfo(BrowserInformation.newBuilder().setColorDepth(24).setScreenHeight(900).setScreenWidth(1440).setJavaEnabled(false).setJavaScriptEnabled(true).setLanguage("en-US").setTimeZoneOffsetMinutes(-480).setAcceptHeader("application/json").setUserAgent("Mozilla/5.0 (probe-bot)").setAcceptLanguage("en-US,en;q=0.9").setIpAddress("1.2.3.4").build()).build())
    println("[setup_recurring] HTTP ${result1.statusCode}")

    // Step 2: Recurring Charge — charge against the stored mandate
    val result2 = recurringPaymentClient.charge(RecurringPaymentServiceChargeRequest.newBuilder().setConnectorRecurringPaymentId(MandateReference.newBuilder().setConnectorMandateId(ConnectorMandateReferenceId.newBuilder().setConnectorMandateId("probe-mandate-123").build()).build()).setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setToken(TokenPaymentMethodType.newBuilder().setToken(SecretString.newBuilder().setValue("probe_pm_token").build()).build()).build()).setReturnUrl("https://example.com/recurring-return").setConnectorCustomerId("cust_probe_123").setPaymentMethodType(PaymentMethodType.PAY_PAL).setOffSession(true).build())
    println("[recurring_charge] HTTP ${result2.statusCode}")

    return mapOf("statusCode" to result2.statusCode)
}
fun processVoidPayment(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Void a Payment
    val directPaymentClient = DirectPaymentClient(config)

    // Step 1: Authorize — reserve funds on the payment method
    val result1 = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.MANUAL).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setBrowserInfo(BrowserInformation.newBuilder().setColorDepth(24).setScreenHeight(900).setScreenWidth(1440).setJavaEnabled(false).setJavaScriptEnabled(true).setLanguage("en-US").setTimeZoneOffsetMinutes(-480).setAcceptHeader("application/json").setUserAgent("Mozilla/5.0 (probe-bot)").setAcceptLanguage("en-US,en;q=0.9").setIpAddress("1.2.3.4").build()).build())
    println("[authorize] HTTP ${result1.statusCode}")

    // Step 2: Void — release reserved funds (cancel authorization)
    val result2 = directPaymentClient.void(PaymentServiceVoidRequest.newBuilder().setMerchantVoidId("probe_void_001").setConnectorTransactionId("probe_connector_txn_001").build())
    println("[void] HTTP ${result2.statusCode}")

    return mapOf("statusCode" to result2.statusCode)
}
fun authorize(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.authorize (Card)
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.authorize(PaymentServiceAuthorizeRequest.newBuilder().setMerchantTransactionId("probe_txn_001").setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCaptureMethod(CaptureMethod.AUTOMATIC).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setReturnUrl("https://example.com/return").setBrowserInfo(BrowserInformation.newBuilder().setColorDepth(24).setScreenHeight(900).setScreenWidth(1440).setJavaEnabled(false).setJavaScriptEnabled(true).setLanguage("en-US").setTimeZoneOffsetMinutes(-480).setAcceptHeader("application/json").setUserAgent("Mozilla/5.0 (probe-bot)").setAcceptLanguage("en-US,en;q=0.9").setIpAddress("1.2.3.4").build()).build())
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
fun dispute_accept(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: DisputeService.dispute_accept
    val disputeClient = DisputeClient(config)

    val result = disputeClient.accept(DisputeServiceAcceptRequest.newBuilder().setMerchantDisputeId("probe_dispute_001").setConnectorTransactionId("probe_txn_001").setDisputeId("probe_dispute_id_001").build())
    println("[dispute_accept] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun dispute_defend(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: DisputeService.dispute_defend
    val disputeClient = DisputeClient(config)

    val result = disputeClient.defend(DisputeServiceDefendRequest.newBuilder().setMerchantDisputeId("probe_dispute_001").setConnectorTransactionId("probe_txn_001").setDisputeId("probe_dispute_id_001").setReasonCode("probe_reason").build())
    println("[dispute_defend] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun dispute_submit_evidence(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: DisputeService.dispute_submit_evidence
    val disputeClient = DisputeClient(config)

    val result = disputeClient.submit_evidence(DisputeServiceSubmitEvidenceRequest.newBuilder().setMerchantDisputeId("probe_dispute_001").setConnectorTransactionId("probe_txn_001").setDisputeId("probe_dispute_id_001").setEvidenceDocuments([{'evidence_type': 'SERVICE_DOCUMENTATION', 'file_content': [112, 114, 111, 98, 101, 32, 101, 118, 105, 100, 101, 110, 99, 101, 32, 99, 111, 110, 116, 101, 110, 116], 'file_mime_type': 'application/pdf'}]).build())
    println("[dispute_submit_evidence] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun recurring_charge(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: RecurringPaymentService.recurring_charge
    val recurringPaymentClient = RecurringPaymentClient(config)

    val result = recurringPaymentClient.charge(RecurringPaymentServiceChargeRequest.newBuilder().setConnectorRecurringPaymentId(MandateReference.newBuilder().setConnectorMandateId(ConnectorMandateReferenceId.newBuilder().setConnectorMandateId("probe-mandate-123").build()).build()).setAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setToken(TokenPaymentMethodType.newBuilder().setToken(SecretString.newBuilder().setValue("probe_pm_token").build()).build()).build()).setReturnUrl("https://example.com/recurring-return").setConnectorCustomerId("cust_probe_123").setPaymentMethodType(PaymentMethodType.PAY_PAL).setOffSession(true).build())
    println("[recurring_charge] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun refund(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.refund
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.refund(PaymentServiceRefundRequest.newBuilder().setMerchantRefundId("probe_refund_001").setConnectorTransactionId("probe_connector_txn_001").setPaymentAmount(1000).setRefundAmount(Money.newBuilder().setMinorAmount(1000).setCurrency(Currency.USD).build()).setReason("customer_request").build())
    println("[refund] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun setup_recurring(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.setup_recurring
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.setup_recurring(PaymentServiceSetupRecurringRequest.newBuilder().setMerchantRecurringPaymentId("probe_mandate_001").setAmount(Money.newBuilder().setMinorAmount(0).setCurrency(Currency.USD).build()).setPaymentMethod(PaymentMethod.newBuilder().setCard(CardDetails.newBuilder().setCardNumber(CardNumberType.newBuilder().setValue("4111111111111111").build()).setCardExpMonth(SecretString.newBuilder().setValue("03").build()).setCardExpYear(SecretString.newBuilder().setValue("2030").build()).setCardCvc(SecretString.newBuilder().setValue("737").build()).setCardHolderName(SecretString.newBuilder().setValue("John Doe").build()).build()).build()).setCustomer(Customer.newBuilder().setId("cust_probe_123").build()).setAddress(PaymentAddress.newBuilder().setBillingAddress(Address.newBuilder().build()).build()).setAuthType(AuthenticationType.NO_THREE_DS).setEnrolledFor3Ds(false).setReturnUrl("https://example.com/mandate-return").setSetupFutureUsage(FutureUsage.OFF_SESSION).setRequestIncrementalAuthorization(false).setCustomerAcceptance(CustomerAcceptance.newBuilder().setAcceptanceType(AcceptanceType.OFFLINE).setAcceptedAt(0).build()).setBrowserInfo(BrowserInformation.newBuilder().setColorDepth(24).setScreenHeight(900).setScreenWidth(1440).setJavaEnabled(false).setJavaScriptEnabled(true).setLanguage("en-US").setTimeZoneOffsetMinutes(-480).setAcceptHeader("application/json").setUserAgent("Mozilla/5.0 (probe-bot)").setAcceptLanguage("en-US,en;q=0.9").setIpAddress("1.2.3.4").build()).build())
    println("[setup_recurring] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}
fun void(txnId: String, config: ConnectorConfig): Map<String, Any?> {
    // Flow: PaymentService.void
    val directPaymentClient = DirectPaymentClient(config)

    val result = directPaymentClient.void(PaymentServiceVoidRequest.newBuilder().setMerchantVoidId("probe_void_001").setConnectorTransactionId("probe_connector_txn_001").build())
    println("[void] HTTP ${result.statusCode}")
    return mapOf("statusCode" to result.statusCode)
}