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

        val codeSource = GrpcFfi::class.java.protectionDomain.codeSource
        if (codeSource != null) {
            val jarDir = File(codeSource.location.toURI()).parentFile
            val nativeDir = File(jarDir, "native")
            val libFile = File(nativeDir, libName)
            if (libFile.exists()) return libFile.absolutePath
        }

        val resource = GrpcFfi::class.java.classLoader.getResource("native/$libName")
        if (resource != null) return File(resource.toURI()).absolutePath

        return libName.removePrefix("lib")
    }

    fun call(method: String, configBytes: ByteArray, reqBytes: ByteArray): ByteArray {
        val configMem = Memory(configBytes.size.toLong().coerceAtLeast(1))
        configMem.write(0, configBytes, 0, configBytes.size)

        val reqMem = Memory(reqBytes.size.toLong().coerceAtLeast(1))
        reqMem.write(0, reqBytes, 0, reqBytes.size)

        val outLen = IntByReference(0)

        val resultPtr = lib.hyperswitch_grpc_call(
            method, configMem, configBytes.size, reqMem, reqBytes.size, outLen
        )

        val len = outLen.value
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
     * DisputeService.Get — Retrieve dispute status and evidence submission state. Tracks dispute progress through bank review process for informed decision-making.
     */
    suspend fun dispute_get(req: DisputeServiceGetRequest): DisputeResponse =
        callGrpc(config, "dispute/dispute_get", req, DisputeResponse.parser())
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
     * MerchantAuthenticationService.CreateServerAuthenticationToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.
     */
    suspend fun create_server_authentication_token(req: MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest): MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse =
        callGrpc(config, "merchant_authentication/create_server_authentication_token", req, MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse.parser())
    /**
     * MerchantAuthenticationService.CreateServerSessionAuthenticationToken — Create a server-side session with the connector. Establishes session state for multi-step operations like 3DS verification or wallet authorization.
     */
    suspend fun create_server_session_authentication_token(req: MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest): MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse =
        callGrpc(config, "merchant_authentication/create_server_session_authentication_token", req, MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse.parser())
    /**
     * MerchantAuthenticationService.CreateClientAuthenticationToken — Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.
     */
    suspend fun create_client_authentication_token(req: MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest): MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse =
        callGrpc(config, "merchant_authentication/create_client_authentication_token", req, MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse.parser())
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
     * PaymentService.Void — Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.
     */
    suspend fun void(req: PaymentServiceVoidRequest): PaymentServiceVoidResponse =
        callGrpc(config, "payment/void", req, PaymentServiceVoidResponse.parser())
    /**
     * PaymentService.Reverse — Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.
     */
    suspend fun reverse(req: PaymentServiceReverseRequest): PaymentServiceReverseResponse =
        callGrpc(config, "payment/reverse", req, PaymentServiceReverseResponse.parser())
    /**
     * PaymentService.Capture — Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.
     */
    suspend fun capture(req: PaymentServiceCaptureRequest): PaymentServiceCaptureResponse =
        callGrpc(config, "payment/capture", req, PaymentServiceCaptureResponse.parser())
    /**
     * PaymentService.CreateOrder — Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.
     */
    suspend fun create_order(req: PaymentServiceCreateOrderRequest): PaymentServiceCreateOrderResponse =
        callGrpc(config, "payment/create_order", req, PaymentServiceCreateOrderResponse.parser())
    /**
     * PaymentService.Refund — Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.
     */
    suspend fun refund(req: PaymentServiceRefundRequest): RefundResponse =
        callGrpc(config, "payment/refund", req, RefundResponse.parser())
    /**
     * PaymentService.IncrementalAuthorization — Increase the authorized amount for an existing payment. Enables you to capture additional funds when the transaction amount changes after initial authorization.
     */
    suspend fun incremental_authorization(req: PaymentServiceIncrementalAuthorizationRequest): PaymentServiceIncrementalAuthorizationResponse =
        callGrpc(config, "payment/incremental_authorization", req, PaymentServiceIncrementalAuthorizationResponse.parser())
    /**
     * PaymentService.VerifyRedirectResponse — Verify and process redirect responses from 3D Secure or other external flows. Validates authentication results and updates payment state accordingly.
     */
    suspend fun verify_redirect_response(req: PaymentServiceVerifyRedirectResponseRequest): PaymentServiceVerifyRedirectResponseResponse =
        callGrpc(config, "payment/verify_redirect_response", req, PaymentServiceVerifyRedirectResponseResponse.parser())
    /**
     * PaymentService.SetupRecurring — Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.
     */
    suspend fun setup_recurring(req: PaymentServiceSetupRecurringRequest): PaymentServiceSetupRecurringResponse =
        callGrpc(config, "payment/setup_recurring", req, PaymentServiceSetupRecurringResponse.parser())
    /**
     * PaymentService.TokenAuthorize — Authorize using a connector-issued payment method token.
     */
    suspend fun token_authorize(req: PaymentServiceTokenAuthorizeRequest): PaymentServiceAuthorizeResponse =
        callGrpc(config, "payment/token_authorize", req, PaymentServiceAuthorizeResponse.parser())
    /**
     * PaymentService.TokenSetupRecurring — Setup a recurring mandate using a connector token.
     */
    suspend fun token_setup_recurring(req: PaymentServiceTokenSetupRecurringRequest): PaymentServiceSetupRecurringResponse =
        callGrpc(config, "payment/token_setup_recurring", req, PaymentServiceSetupRecurringResponse.parser())
    /**
     * PaymentService.ProxyAuthorize — Authorize using vault-aliased card data. Proxy substitutes before connector.
     */
    suspend fun proxy_authorize(req: PaymentServiceProxyAuthorizeRequest): PaymentServiceAuthorizeResponse =
        callGrpc(config, "payment/proxy_authorize", req, PaymentServiceAuthorizeResponse.parser())
    /**
     * PaymentService.ProxySetupRecurring — Setup recurring mandate using vault-aliased card data.
     */
    suspend fun proxy_setup_recurring(req: PaymentServiceProxySetupRecurringRequest): PaymentServiceSetupRecurringResponse =
        callGrpc(config, "payment/proxy_setup_recurring", req, PaymentServiceSetupRecurringResponse.parser())
}

/**
 * PayoutService — gRPC sub-client.
 */
class GrpcPayoutClient internal constructor(
    private val config: GrpcConfig,
) {
    /**
     * PayoutService.Create — Creates a payout.
     */
    suspend fun payout_create(req: PayoutServiceCreateRequest): PayoutServiceCreateResponse =
        callGrpc(config, "payout/payout_create", req, PayoutServiceCreateResponse.parser())
    /**
     * PayoutService.Transfer — Creates a payout fund transfer.
     */
    suspend fun transfer(req: PayoutServiceTransferRequest): PayoutServiceTransferResponse =
        callGrpc(config, "payout/transfer", req, PayoutServiceTransferResponse.parser())
    /**
     * PayoutService.Get — Retrieve payout details.
     */
    suspend fun payout_get(req: PayoutServiceGetRequest): PayoutServiceGetResponse =
        callGrpc(config, "payout/payout_get", req, PayoutServiceGetResponse.parser())
    /**
     * PayoutService.Void — Void a payout.
     */
    suspend fun payout_void(req: PayoutServiceVoidRequest): PayoutServiceVoidResponse =
        callGrpc(config, "payout/payout_void", req, PayoutServiceVoidResponse.parser())
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
    /**
     * RecurringPaymentService.CancelRecurring — Cancel a specific recurring payment under a subscription. Stops a pending or scheduled payment without revoking the entire mandate/subscription.
     */
    suspend fun cancel_recurring(req: RecurringPaymentServiceCancelRecurringRequest): RecurringPaymentServiceCancelRecurringResponse =
        callGrpc(config, "recurring_payment/cancel_recurring", req, RecurringPaymentServiceCancelRecurringResponse.parser())
}

/**
 * RefundService — gRPC sub-client.
 */
class GrpcRefundClient internal constructor(
    private val config: GrpcConfig,
) {
    /**
     * RefundService.Get — Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.
     */
    suspend fun refund_get(req: RefundServiceGetRequest): RefundResponse =
        callGrpc(config, "refund/refund_get", req, RefundResponse.parser())
}

// ── Top-level GrpcClient ──────────────────────────────────────────────────────

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
    val refund: GrpcRefundClient =
        GrpcRefundClient(config)
}
