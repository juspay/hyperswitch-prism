// Card Payment (Authorize + Capture) - Universal Example
//
// Works with any connector that supports card payments.
// Usage: ./gradlew run --args="checkout_card --connector=stripe"

package examples.scenarios

import payments.PaymentClient
import payments.PaymentServiceAuthorizeRequest
import payments.PaymentServiceCaptureRequest
import payments.ConnectorConfig
import payments.SdkOptions
import payments.Environment
import payments.ConnectorSpecificConfig
import payments.Currency
import payments.CaptureMethod
import com.google.gson.Gson
import com.google.gson.JsonObject
import java.io.File

// [START imports]
// Already imported above
// [END imports]

// [START load_probe_data]
fun loadProbeData(connectorName: String): JsonObject {
    val probePath = File("data/field_probe/$connectorName.json")
    val jsonString = probePath.readText()
    return Gson().fromJson(jsonString, JsonObject::class.java)
}
// [END load_probe_data]

// [START stripe_config]
fun getStripeConfig(apiKey: String): ConnectorConfig {
    return ConnectorConfig.newBuilder().apply {
        options = SdkOptions.newBuilder().apply {
            environment = Environment.SANDBOX
        }.build()
        connectorConfig = ConnectorSpecificConfig.newBuilder().apply {
            stripeBuilder.apply {
                this.apiKey = apiKey
            }
        }.build()
    }.build()
}
// [END stripe_config]

// [START adyen_config]
fun getAdyenConfig(apiKey: String, merchantAccount: String): ConnectorConfig {
    return ConnectorConfig.newBuilder().apply {
        options = SdkOptions.newBuilder().apply {
            environment = Environment.SANDBOX
        }.build()
        connectorConfig = ConnectorSpecificConfig.newBuilder().apply {
            adyenBuilder.apply {
                this.apiKey = apiKey
                this.merchantAccount = merchantAccount
            }
        }.build()
    }.build()
}
// [END adyen_config]

// [START checkout_config]
fun getCheckoutConfig(apiKey: String): ConnectorConfig {
    return ConnectorConfig.newBuilder().apply {
        options = SdkOptions.newBuilder().apply {
            environment = Environment.SANDBOX
        }.build()
        connectorConfig = ConnectorSpecificConfig.newBuilder().apply {
            checkoutBuilder.apply {
                this.apiKey = apiKey
            }
        }.build()
    }.build()
}
// [END checkout_config]

// [START get_connector_config]
fun getConnectorConfig(connectorName: String, credentials: Map<String, String>): ConnectorConfig {
    return when (connectorName) {
        "stripe" -> getStripeConfig(credentials["apiKey"] ?: throw IllegalArgumentException("apiKey required"))
        "adyen" -> getAdyenConfig(
            credentials["apiKey"] ?: throw IllegalArgumentException("apiKey required"),
            credentials["merchantAccount"] ?: throw IllegalArgumentException("merchantAccount required")
        )
        "checkout" -> getCheckoutConfig(credentials["apiKey"] ?: throw IllegalArgumentException("apiKey required"))
        else -> throw IllegalArgumentException("Unknown connector: $connectorName")
    }
}
// [END get_connector_config]

// [START build_authorize_request]
fun buildAuthorizeRequest(probeData: JsonObject, captureMethod: CaptureMethod = CaptureMethod.MANUAL): PaymentServiceAuthorizeRequest {
    val flows = probeData.getAsJsonObject("flows")
    val authorizeFlows = flows?.getAsJsonObject("authorize")
        ?: throw IllegalStateException("Connector doesn't support authorize flow")
    
    // Find Card payment method or first supported
    var cardData: JsonObject? = null
    for ((pmKey, pmData) in authorizeFlows.entrySet()) {
        val pmObj = pmData.asJsonObject
        if (pmObj.get("status")?.asString == "supported") {
            if (pmKey == "Card") {
                cardData = pmObj
                break
            } else if (cardData == null) {
                cardData = pmObj
            }
        }
    }
    
    cardData ?: throw IllegalStateException("No supported payment method found")
    val protoRequest = cardData.getAsJsonObject("proto_request")
        ?: throw IllegalStateException("No proto_request in probe data")
    
    // Build request from proto_request
    return PaymentServiceAuthorizeRequest.newBuilder().apply {
        merchantTransactionId = protoRequest.get("merchant_transaction_id")?.asString ?: "txn_001"
        amountBuilder.apply {
            minorAmount = protoRequest.getAsJsonObject("amount")?.get("minor_amount")?.asLong ?: 1000
            currency = Currency.valueOf(protoRequest.getAsJsonObject("amount")?.get("currency")?.asString ?: "USD")
        }
        this.captureMethod = captureMethod
        // Add other fields as needed
    }.build()
}
// [END build_authorize_request]

// [START build_capture_request]
fun buildCaptureRequest(
    connectorTransactionId: String,
    amount: payments.Money,
    merchantCaptureId: String = "capture_001"
): PaymentServiceCaptureRequest {
    return PaymentServiceCaptureRequest.newBuilder().apply {
        this.merchantCaptureId = merchantCaptureId
        this.connectorTransactionId = connectorTransactionId
        amountToCapture = amount
    }.build()
}
// [END build_capture_request]

// [START process_checkout_card]
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
// [END process_checkout_card]

// [START main]
fun main(args: Array<String>) {
    val connectorArg = args.find { it.startsWith("--connector=") }
    val credsArg = args.find { it.startsWith("--credentials=") }
    
    if (connectorArg == null) {
        println("Error: --connector is required")
        return
    }
    
    val connectorName = connectorArg.substringAfter("=")
    
    // Load credentials
    val credentials = if (credsArg != null) {
        val credsPath = credsArg.substringAfter("=")
        val jsonString = File(credsPath).readText()
        Gson().fromJson(jsonString, Map::class.java) as Map<String, String>
    } else {
        println("⚠️  Using dummy credentials. Set --credentials for real API calls.")
        mapOf("apiKey" to "sk_test_dummy")
    }
    
    // Run the flow
    val result = processCheckoutCard(connectorName, credentials)
    
    println("\n${"=".repeat(60)}")
    println("Result: $result")
    println("${"=".repeat(60)}")
    
    if (result != "success" && result != "pending") {
        System.exit(1)
    }
}
// [END main]
