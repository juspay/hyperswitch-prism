// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py netcetera
//
// Netcetera — all scenarios and flows in one file.
// Run a scenario:  ./gradlew run --args="netcetera processCheckoutCard"

package examples.netcetera

import types.Payment.*
import types.Events.*
import types.PaymentMethods.*
import payments.PaymentMethodAuthenticationClient
import payments.Currency
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import types.Payment.NetceteraConfig
import payments.SecretString

val SUPPORTED_FLOWS = listOf<String>("authenticate", "post_authenticate", "pre_authenticate")

val _defaultConfig: ConnectorConfig = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setNetcetera(NetceteraConfig.newBuilder()
                .setCertificate(SecretString.newBuilder().setValue("YOUR_CERTIFICATE").build())
                .setPrivateKey(SecretString.newBuilder().setValue("YOUR_PRIVATE_KEY").build())
                .setThreeDsRequestorId("YOUR_THREE_DS_REQUESTOR_ID")
                .setThreeDsRequestorName("YOUR_THREE_DS_REQUESTOR_NAME")
                .setMerchantConfigurationId("YOUR_MERCHANT_CONFIGURATION_ID")
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()


// Flow: PaymentMethodAuthenticationService.Authenticate
fun authenticate(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentMethodAuthenticationClient(config)
    val request = PaymentMethodAuthenticationServiceAuthenticateRequest.newBuilder().apply {
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        paymentMethodBuilder.apply {  // Payment Method.
            cardBuilder.apply {  // Generic card payment.
                cardNumberBuilder.value = "4111111111111111"  // Card Identification.
                cardExpMonthBuilder.value = "03"
                cardExpYearBuilder.value = "2030"
                cardCvcBuilder.value = "737"
                cardHolderNameBuilder.value = "John Doe"  // Cardholder Information.
            }
        }
        addressBuilder.apply {  // Address Information.
            billingAddressBuilder.apply {
            }
        }
        returnUrl = "https://example.com/3ds-return"  // URLs for Redirection. For 3DS this is the browser challenge return URL (EMVCo notificationURL / threeDSRequestorURL).
    }.build()
    val response = client.authenticate(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentMethodAuthenticationService.PostAuthenticate
fun postAuthenticate(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentMethodAuthenticationClient(config)
    val request = PaymentMethodAuthenticationServicePostAuthenticateRequest.newBuilder().apply {
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        paymentMethodBuilder.apply {  // Payment Method.
            cardBuilder.apply {  // Generic card payment.
                cardNumberBuilder.value = "4111111111111111"  // Card Identification.
                cardExpMonthBuilder.value = "03"
                cardExpYearBuilder.value = "2030"
                cardCvcBuilder.value = "737"
                cardHolderNameBuilder.value = "John Doe"  // Cardholder Information.
            }
        }
        addressBuilder.apply {  // Address Information.
            billingAddressBuilder.apply {
            }
        }
    }.build()
    val response = client.post_authenticate(request)
    println("Status: ${response.status.name}")
}

// Flow: PaymentMethodAuthenticationService.PreAuthenticate
fun preAuthenticate(txnId: String, config: ConnectorConfig = _defaultConfig) {
    val client = PaymentMethodAuthenticationClient(config)
    val request = PaymentMethodAuthenticationServicePreAuthenticateRequest.newBuilder().apply {
        amountBuilder.apply {  // Amount Information.
            minorAmount = 1000L  // Amount in minor units (e.g., 1000 = $10.00).
            currency = Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
        paymentMethodBuilder.apply {  // Payment Method.
            cardBuilder.apply {  // Generic card payment.
                cardNumberBuilder.value = "4111111111111111"  // Card Identification.
                cardExpMonthBuilder.value = "03"
                cardExpYearBuilder.value = "2030"
                cardCvcBuilder.value = "737"
                cardHolderNameBuilder.value = "John Doe"  // Cardholder Information.
            }
        }
        addressBuilder.apply {  // Address Information.
            billingAddressBuilder.apply {
            }
        }
        enrolledFor3Ds = false  // Authentication Details.
        returnUrl = "https://example.com/3ds-return"  // URLs for Redirection.
    }.build()
    val response = client.pre_authenticate(request)
    println("Status: ${response.status.name}")
}


fun main(args: Array<String>) {
    val txnId = "order_001"
    val flow = args.firstOrNull() ?: "authenticate"
    when (flow) {
        "authenticate" -> authenticate(txnId)
        "postAuthenticate" -> postAuthenticate(txnId)
        "preAuthenticate" -> preAuthenticate(txnId)
        else -> System.err.println("Unknown flow: $flow. Available: authenticate, postAuthenticate, preAuthenticate")
    }
}
