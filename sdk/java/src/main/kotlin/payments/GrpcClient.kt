// AUTO-GENERATED — do not edit by hand.
// Source: services.proto  |  Regenerate: make generate  (or: python3 scripts/generators/code/generate.py --lang kotlin)

package payments

import com.google.protobuf.MessageLite
import com.google.protobuf.Parser
import com.sun.jna.Library
import com.sun.jna.Memory
import com.sun.jna.Native
import com.sun.jna.Pointer
import com.sun.jna.ptr.IntByReference
import java.io.File
import java.nio.charset.StandardCharsets
import types.Payment.*
import types.PaymentMethods.*
import types.Payouts.*

// ── Config ────────────────────────────────────────────────────────────────────

/**
 * Connection configuration for the gRPC client.
 *
 * The connector_config field should contain the connector-specific authentication
 * and configuration in the format expected by the server:
 * {"config": {"ConnectorName": {"api_key": "...", ...}}}
 */
data class GrpcConfig(
    val endpoint: String,
    val connector: String,
    val connectorConfig: Map<String, Any>,
)

// ── JNA FFI bindings ──────────────────────────────────────────────────────────

private interface GrpcFfiLib : Library {
    fun hyperswitch_grpc_call(
        method: String,
        configPtr: Pointer,
        configLen: Int,
        reqPtr: Pointer,
        reqLen: Int,
        outLen: IntByReference,
    ): Pointer

    fun hyperswitch_grpc_free(ptr: Pointer, len: Int)
}

private object GrpcFfi {
    private val lib: GrpcFfiLib by lazy {
        val libPath = System.getProperty("hyperswitch.grpc.lib.path")
            ?: detectLibPath()
        Native.load(libPath, GrpcFfiLib::class.java)
    }

    private fun detectLibPath(): String {
        val os = System.getProperty("os.name").lowercase()
        val ext = when {
            os.contains("mac") || os.contains("darwin") -> "dylib"
            else -> "so"
        }
        val libName = "libhyperswitch_grpc_ffi.$ext"

        // Try relative to jar location
        val codeSource = GrpcFfi::class.java.protectionDomain.codeSource
        if (codeSource != null) {
            val jarDir = File(codeSource.location.toURI()).parentFile
            val nativeDir = File(jarDir, "native")
            val libFile = File(nativeDir, libName)
            if (libFile.exists()) return libFile.absolutePath
        }

        // Try classpath resource
        val resource = GrpcFfi::class.java.classLoader.getResource("native/$libName")
        if (resource != null) return File(resource.toURI()).absolutePath

        // Fallback to system library path
        return libName.removePrefix("lib")
    }

    fun call(method: String, configBytes: ByteArray, reqBytes: ByteArray): ByteArray {
        // Use JNA Memory to allocate native buffers — avoids GetDirectBufferAddress
        // which is restricted in unnamed modules on Java 22+.
        val configMem = Memory(configBytes.size.toLong().coerceAtLeast(1))
        configMem.write(0, configBytes, 0, configBytes.size)

        val reqMem = Memory(reqBytes.size.toLong().coerceAtLeast(1))
        reqMem.write(0, reqBytes, 0, reqBytes.size)

        val outLen = IntByReference(0)

        val resultPtr = lib.hyperswitch_grpc_call(
            method, configMem, configBytes.size, reqMem, reqBytes.size, outLen
        )

        val len = outLen.value

        // Read bytes from pointer and copy to heap array (free immediately after read)
        val raw = ByteArray(len)
        resultPtr.read(0, raw, 0, len)
        lib.hyperswitch_grpc_free(resultPtr, len)
        return raw
    }
}

// ── Dispatch helper ───────────────────────────────────────────────────────────

private fun <T : com.google.protobuf.MessageLite> callGrpc(
    config: GrpcConfig,
    method: String,
    req: com.google.protobuf.MessageLite,
    responseParser: com.google.protobuf.Parser<T>,
): T {
    val configJson = mapOf(
        "endpoint" to config.endpoint,
        "connector" to config.connector,
        "connector_config" to config.connectorConfig,
    )
    val configBytes = com.google.gson.Gson().toJson(configJson).toByteArray(StandardCharsets.UTF_8)
    val reqBytes = req.toByteArray()

    val raw = GrpcFfi.call(method, configBytes, reqBytes)

    if (raw.isEmpty()) {
        throw RuntimeException("gRPC error ($method): empty response")
    }

    if (raw[0] == 1.toByte()) {
        val errorMsg = String(raw, 1, raw.size - 1, StandardCharsets.UTF_8)
        throw RuntimeException("gRPC error ($method): $errorMsg")
    }

    return responseParser.parseFrom(raw.slice(1 until raw.size).toByteArray())
}

// ── Sub-clients (one per proto service) ───────────────────────────────────────

/**
 * CustomerService — gRPC sub-client.
 */
class GrpcCustomerClient internal constructor(
    private val config: GrpcConfig,
) {
    /**
     * CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.
     */
    suspend fun create(req: CustomerServiceCreateRequest): CustomerServiceCreateResponse =
        callGrpc(config, "customer/create", req, CustomerServiceCreateResponse.parser())

}

/**
 * DisputeService — gRPC sub-client.
 */
class GrpcDisputeClient internal constructor(
    private val config: GrpcConfig,
) {
    /**
     * DisputeService.SubmitEvidence — Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims.
     */
    suspend fun submit_evidence(req: DisputeServiceSubmitEvidenceRequest): DisputeServiceSubmitEvidenceResponse =
        callGrpc(config, "dispute/submit_evidence", req, DisputeServiceSubmitEvidenceResponse.parser())

    /**
     * DisputeService.Defend — Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation.
     */
    suspend fun defend(req: DisputeServiceDefendRequest): DisputeServiceDefendResponse =
        callGrpc(config, "dispute/defend", req, DisputeServiceDefendResponse.parser())

    /**
     * DisputeService.Accept — Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient.
     */
    suspend fun accept(req: DisputeServiceAcceptRequest): DisputeServiceAcceptResponse =
        callGrpc(config, "dispute/accept", req, DisputeServiceAcceptResponse.parser())

}

/**
 * EventService — gRPC sub-client.
 */
class GrpcEventClient internal constructor(
    private val config: GrpcConfig,
) {
    /**
     * EventService.HandleEvent — Process webhook notifications from connectors. Translates connector events into standardized responses for asynchronous payment state updates.
     */
    suspend fun handle_event(req: EventServiceHandleRequest): EventServiceHandleResponse =
        callGrpc(config, "event/handle_event", req, EventServiceHandleResponse.parser())

}

/**
 * MerchantAuthenticationService — gRPC sub-client.
 */
class GrpcMerchantAuthenticationClient internal constructor(
    private val config: GrpcConfig,
) {
    /**
     * MerchantAuthenticationService.CreateAccessToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.
     */
    suspend fun create_access_token(req: MerchantAuthenticationServiceCreateAccessTokenRequest): MerchantAuthenticationServiceCreateAccessTokenResponse =
        callGrpc(config, "merchant_authentication/create_access_token", req, MerchantAuthenticationServiceCreateAccessTokenResponse.parser())

    /**
     * MerchantAuthenticationService.CreateSessionToken — Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking.
     */
    suspend fun create_session_token(req: MerchantAuthenticationServiceCreateSessionTokenRequest): MerchantAuthenticationServiceCreateSessionTokenResponse =
        callGrpc(config, "merchant_authentication/create_session_token", req, MerchantAuthenticationServiceCreateSessionTokenResponse.parser())

    /**
     * MerchantAuthenticationService.CreateSdkSessionToken — Initialize wallet payment sessions for Apple Pay, Google Pay, etc. Sets up secure context for tokenized wallet payments with device verification.
     */
    suspend fun create_sdk_session_token(req: MerchantAuthenticationServiceCreateSdkSessionTokenRequest): MerchantAuthenticationServiceCreateSdkSessionTokenResponse =
        callGrpc(config, "merchant_authentication/create_sdk_session_token", req, MerchantAuthenticationServiceCreateSdkSessionTokenResponse.parser())

}

/**
 * PaymentMethodAuthenticationService — gRPC sub-client.
 */
class GrpcPaymentMethodAuthenticationClient internal constructor(
    private val config: GrpcConfig,
) {
    /**
     * PaymentMethodAuthenticationService.PreAuthenticate — Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.
     */
    suspend fun pre_authenticate(req: PaymentMethodAuthenticationServicePreAuthenticateRequest): PaymentMethodAuthenticationServicePreAuthenticateResponse =
        callGrpc(config, "payment_method_authentication/pre_authenticate", req, PaymentMethodAuthenticationServicePreAuthenticateResponse.parser())

    /**
     * PaymentMethodAuthenticationService.Authenticate — Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.
     */
    suspend fun authenticate(req: PaymentMethodAuthenticationServiceAuthenticateRequest): PaymentMethodAuthenticationServiceAuthenticateResponse =
        callGrpc(config, "payment_method_authentication/authenticate", req, PaymentMethodAuthenticationServiceAuthenticateResponse.parser())

    /**
     * PaymentMethodAuthenticationService.PostAuthenticate — Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed.
     */
    suspend fun post_authenticate(req: PaymentMethodAuthenticationServicePostAuthenticateRequest): PaymentMethodAuthenticationServicePostAuthenticateResponse =
        callGrpc(config, "payment_method_authentication/post_authenticate", req, PaymentMethodAuthenticationServicePostAuthenticateResponse.parser())

}

/**
 * PaymentMethodService — gRPC sub-client.
 */
class GrpcPaymentMethodClient internal constructor(
    private val config: GrpcConfig,
) {
    /**
     * PaymentMethodService.Tokenize — Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.
     */
    suspend fun tokenize(req: PaymentMethodServiceTokenizeRequest): PaymentMethodServiceTokenizeResponse =
        callGrpc(config, "payment_method/tokenize", req, PaymentMethodServiceTokenizeResponse.parser())

    /**
     * PaymentMethodService.Eligibility — Check if the payout method is eligible for the transaction
     */
    suspend fun eligibility(req: PayoutMethodEligibilityRequest): PayoutMethodEligibilityResponse =
        callGrpc(config, "payment_method/eligibility", req, PayoutMethodEligibilityResponse.parser())

}

/**
 * PaymentService — gRPC sub-client.
 */
class GrpcPaymentClient internal constructor(
    private val config: GrpcConfig,
) {
    /**
     * PaymentService.Authorize — Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing.
     */
    suspend fun authorize(req: PaymentServiceAuthorizeRequest): PaymentServiceAuthorizeResponse =
        callGrpc(config, "payment/authorize", req, PaymentServiceAuthorizeResponse.parser())

    /**
     * PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.
     */
    suspend fun get(req: PaymentServiceGetRequest): PaymentServiceGetResponse =
        callGrpc(config, "payment/get", req, PaymentServiceGetResponse.parser())

    /**
     * PaymentService.Void — Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.
     */
    suspend fun void(req: PaymentServiceVoidRequest): PaymentServiceVoidResponse =
        callGrpc(config, "payment/void", req, PaymentServiceVoidResponse.parser())

    /**
     * PaymentService.Reverse — Reverse a captured payment before settlement. Recovers funds after capture but before bank settlement, used for corrections or cancellations.
     */
    suspend fun reverse(req: PaymentServiceReverseRequest): PaymentServiceReverseResponse =
        callGrpc(config, "payment/reverse", req, PaymentServiceReverseResponse.parser())

    /**
     * PaymentService.Capture — Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.
     */
    suspend fun capture(req: PaymentServiceCaptureRequest): PaymentServiceCaptureResponse =
        callGrpc(config, "payment/capture", req, PaymentServiceCaptureResponse.parser())

    /**
     * PaymentService.CreateOrder — Initialize an order in the payment processor system. Sets up payment context before customer enters card details for improved authorization rates.
     */
    suspend fun create_order(req: PaymentServiceCreateOrderRequest): PaymentServiceCreateOrderResponse =
        callGrpc(config, "payment/create_order", req, PaymentServiceCreateOrderResponse.parser())

    /**
     * PaymentService.Refund — Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.
     */
    suspend fun refund(req: PaymentServiceRefundRequest): RefundResponse =
        callGrpc(config, "payment/refund", req, RefundResponse.parser())

    /**
     * PaymentService.IncrementalAuthorization — Increase authorized amount if still in authorized state. Allows adding charges to existing authorization for hospitality, tips, or incremental services.
     */
    suspend fun incremental_authorization(req: PaymentServiceIncrementalAuthorizationRequest): PaymentServiceIncrementalAuthorizationResponse =
        callGrpc(config, "payment/incremental_authorization", req, PaymentServiceIncrementalAuthorizationResponse.parser())

    /**
     * PaymentService.VerifyRedirectResponse — Validate redirect-based payment responses. Confirms authenticity of redirect-based payment completions to prevent fraud and tampering.
     */
    suspend fun verify_redirect_response(req: PaymentServiceVerifyRedirectResponseRequest): PaymentServiceVerifyRedirectResponseResponse =
        callGrpc(config, "payment/verify_redirect_response", req, PaymentServiceVerifyRedirectResponseResponse.parser())

    /**
     * PaymentService.SetupRecurring — Setup a recurring payment instruction for future payments/ debits. This could be for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.
     */
    suspend fun setup_recurring(req: PaymentServiceSetupRecurringRequest): PaymentServiceSetupRecurringResponse =
        callGrpc(config, "payment/setup_recurring", req, PaymentServiceSetupRecurringResponse.parser())

}

/**
 * PayoutService — gRPC sub-client.
 */
class GrpcPayoutClient internal constructor(
    private val config: GrpcConfig,
) {
    /**
     * PayoutService.Transfer — Creates a payout fund transfer.
     */
    suspend fun transfer(req: PayoutServiceTransferRequest): PayoutServiceTransferResponse =
        callGrpc(config, "payout/transfer", req, PayoutServiceTransferResponse.parser())

    /**
     * PayoutService.Stage — Stage the payout.
     */
    suspend fun stage(req: PayoutServiceStageRequest): PayoutServiceStageResponse =
        callGrpc(config, "payout/stage", req, PayoutServiceStageResponse.parser())

    /**
     * PayoutService.CreateLink — Creates a link between the recipient and the payout.
     */
    suspend fun create_link(req: PayoutServiceCreateLinkRequest): PayoutServiceCreateLinkResponse =
        callGrpc(config, "payout/create_link", req, PayoutServiceCreateLinkResponse.parser())

    /**
     * PayoutService.CreateRecipient — Create payout recipient.
     */
    suspend fun create_recipient(req: PayoutServiceCreateRecipientRequest): PayoutServiceCreateRecipientResponse =
        callGrpc(config, "payout/create_recipient", req, PayoutServiceCreateRecipientResponse.parser())

    /**
     * PayoutService.EnrollDisburseAccount — Enroll disburse account.
     */
    suspend fun enroll_disburse_account(req: PayoutServiceEnrollDisburseAccountRequest): PayoutServiceEnrollDisburseAccountResponse =
        callGrpc(config, "payout/enroll_disburse_account", req, PayoutServiceEnrollDisburseAccountResponse.parser())

}

/**
 * RecurringPaymentService — gRPC sub-client.
 */
class GrpcRecurringPaymentClient internal constructor(
    private val config: GrpcConfig,
) {
    /**
     * RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.
     */
    suspend fun charge(req: RecurringPaymentServiceChargeRequest): RecurringPaymentServiceChargeResponse =
        callGrpc(config, "recurring_payment/charge", req, RecurringPaymentServiceChargeResponse.parser())

    /**
     * RecurringPaymentService.Revoke — Cancel an existing recurring payment mandate. Stops future automatic charges on customer's stored consent for subscription cancellations.
     */
    suspend fun revoke(req: RecurringPaymentServiceRevokeRequest): RecurringPaymentServiceRevokeResponse =
        callGrpc(config, "recurring_payment/revoke", req, RecurringPaymentServiceRevokeResponse.parser())

}

// ── Top-level GrpcClient ──────────────────────────────────────────────────────

/**
 * Top-level gRPC client for the connector-service.
 *
 * Each sub-client corresponds to one proto service. Auth headers from
 * `GrpcConfig` are injected automatically on every call via the Rust FFI layer.
 *
 * Example:
 * ```kotlin
 * val client = GrpcClient(GrpcConfig(
 *     endpoint = "http://localhost:8000",
 *     connector = "stripe",
 *     connectorConfig = mapOf(
 *         "config" to mapOf(
 *             "Stripe" to mapOf("api_key" to "sk_test_...")
 *         )
 *     )
 * ))
 * val res = client.customer.create(...)
 * val res = client.dispute.submit_evidence(...)
 * val res = client.event.handle_event(...)
 * val res = client.merchant_authentication.create_access_token(...)
 * ```
 */
class GrpcClient(config: GrpcConfig) {
    val customer: GrpcCustomerClient =
        GrpcCustomerClient(config)
    val dispute: GrpcDisputeClient =
        GrpcDisputeClient(config)
    val event: GrpcEventClient =
        GrpcEventClient(config)
    val merchant_authentication: GrpcMerchantAuthenticationClient =
        GrpcMerchantAuthenticationClient(config)
    val payment_method_authentication: GrpcPaymentMethodAuthenticationClient =
        GrpcPaymentMethodAuthenticationClient(config)
    val payment_method: GrpcPaymentMethodClient =
        GrpcPaymentMethodClient(config)
    val payment: GrpcPaymentClient =
        GrpcPaymentClient(config)
    val payout: GrpcPayoutClient =
        GrpcPayoutClient(config)
    val recurring_payment: GrpcRecurringPaymentClient =
        GrpcRecurringPaymentClient(config)
}
