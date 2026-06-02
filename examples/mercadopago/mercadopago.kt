// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py mercadopago
//
// Mercadopago — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="mercadopago processCheckoutCard"

package examples.mercadopago

import types.Payment.*
import types.PaymentMethods.*
import payments.MerchantAuthenticationClient
import payments.PaymentClient
import payments.CaptureMethod
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.MercadopagoConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("create_server_session_authentication_token", "token_authorize")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setMercadopago(MercadopagoConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()


// Flow: MerchantAuthenticationService.CreateServerSessionAuthenticationToken
fun createServerSessionAuthenticationToken(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = MerchantAuthenticationClient(config)
    val request = MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest.newBuilder().apply {
        paymentBuilder.apply {  // PayoutSessionContext payout = 6; // future FrmSessionContext frm = 7; // future.
            amountBuilder.apply {
                minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
                currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
            }
        }
    }.build()
    val response = client.create_server_session_authentication_token(request)
    println("Session token: ${response.sessionToken} (statusCode=${response.statusCode})")
}

// Flow: PaymentService.TokenAuthorize
fun tokenAuthorize(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentClient(config)
    val request = PaymentServiceTokenAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = "probe_tokenized_txn_001"
        amountBuilder.apply {
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        connectorTokenBuilder.value = "pm_1AbcXyzStripeTestToken"  // Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
        addressBuilder.apply {
            billingAddressBuilder.apply {
            }
        }
        captureMethod = CaptureMethod.AUTOMATIC
        returnUrl = "https://example.com/return"
    }.build()
    val response = client.token_authorize(request)
    println("Status: ${response.status.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "createServerSessionAuthenticationToken"
    when (flow) {
        "createServerSessionAuthenticationToken" -> createServerSessionAuthenticationToken(txnId)
        "tokenAuthorize" -> tokenAuthorize(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: createServerSessionAuthenticationToken, tokenAuthorize")
    }
}
