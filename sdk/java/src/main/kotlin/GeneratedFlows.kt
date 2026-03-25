// AUTO-GENERATED — do not edit by hand.
// Source: services.proto ∩ services/payments.rs  |  Regenerate: make generate

package payments

import types.Payment.*
import types.Payouts.*
import types.PaymentMethods.*

import uniffi.connector_service_ffi.acceptReqTransformer
import uniffi.connector_service_ffi.acceptResTransformer
import uniffi.connector_service_ffi.authenticateReqTransformer
import uniffi.connector_service_ffi.authenticateResTransformer
import uniffi.connector_service_ffi.authorizeReqTransformer
import uniffi.connector_service_ffi.authorizeResTransformer
import uniffi.connector_service_ffi.captureReqTransformer
import uniffi.connector_service_ffi.captureResTransformer
import uniffi.connector_service_ffi.chargeReqTransformer
import uniffi.connector_service_ffi.chargeResTransformer
import uniffi.connector_service_ffi.createReqTransformer
import uniffi.connector_service_ffi.createResTransformer
import uniffi.connector_service_ffi.createAccessTokenReqTransformer
import uniffi.connector_service_ffi.createAccessTokenResTransformer
import uniffi.connector_service_ffi.createOrderReqTransformer
import uniffi.connector_service_ffi.createOrderResTransformer
import uniffi.connector_service_ffi.createSessionTokenReqTransformer
import uniffi.connector_service_ffi.createSessionTokenResTransformer
import uniffi.connector_service_ffi.defendReqTransformer
import uniffi.connector_service_ffi.defendResTransformer
import uniffi.connector_service_ffi.getReqTransformer
import uniffi.connector_service_ffi.getResTransformer
import uniffi.connector_service_ffi.payoutCreateReqTransformer
import uniffi.connector_service_ffi.payoutCreateResTransformer
import uniffi.connector_service_ffi.payoutCreateLinkReqTransformer
import uniffi.connector_service_ffi.payoutCreateLinkResTransformer
import uniffi.connector_service_ffi.payoutCreateRecipientReqTransformer
import uniffi.connector_service_ffi.payoutCreateRecipientResTransformer
import uniffi.connector_service_ffi.payoutEnrollDisburseAccountReqTransformer
import uniffi.connector_service_ffi.payoutEnrollDisburseAccountResTransformer
import uniffi.connector_service_ffi.payoutGetReqTransformer
import uniffi.connector_service_ffi.payoutGetResTransformer
import uniffi.connector_service_ffi.payoutStageReqTransformer
import uniffi.connector_service_ffi.payoutStageResTransformer
import uniffi.connector_service_ffi.payoutTransferReqTransformer
import uniffi.connector_service_ffi.payoutTransferResTransformer
import uniffi.connector_service_ffi.payoutVoidReqTransformer
import uniffi.connector_service_ffi.payoutVoidResTransformer
import uniffi.connector_service_ffi.postAuthenticateReqTransformer
import uniffi.connector_service_ffi.postAuthenticateResTransformer
import uniffi.connector_service_ffi.preAuthenticateReqTransformer
import uniffi.connector_service_ffi.preAuthenticateResTransformer
import uniffi.connector_service_ffi.refundReqTransformer
import uniffi.connector_service_ffi.refundResTransformer
import uniffi.connector_service_ffi.reverseReqTransformer
import uniffi.connector_service_ffi.reverseResTransformer
import uniffi.connector_service_ffi.setupRecurringReqTransformer
import uniffi.connector_service_ffi.setupRecurringResTransformer
import uniffi.connector_service_ffi.submitEvidenceReqTransformer
import uniffi.connector_service_ffi.submitEvidenceResTransformer
import uniffi.connector_service_ffi.tokenizeReqTransformer
import uniffi.connector_service_ffi.tokenizeResTransformer
import uniffi.connector_service_ffi.voidReqTransformer
import uniffi.connector_service_ffi.voidResTransformer
import uniffi.connector_service_ffi.handleEventTransformer

object FlowRegistry {
    val reqTransformers: Map<String, (ByteArray, ByteArray) -> ByteArray> = mapOf(
        "accept" to ::acceptReqTransformer,
        "authenticate" to ::authenticateReqTransformer,
        "authorize" to ::authorizeReqTransformer,
        "capture" to ::captureReqTransformer,
        "charge" to ::chargeReqTransformer,
        "create" to ::createReqTransformer,
        "create_access_token" to ::createAccessTokenReqTransformer,
        "create_order" to ::createOrderReqTransformer,
        "create_session_token" to ::createSessionTokenReqTransformer,
        "defend" to ::defendReqTransformer,
        "get" to ::getReqTransformer,
        "payout_create" to ::payoutCreateReqTransformer,
        "payout_create_link" to ::payoutCreateLinkReqTransformer,
        "payout_create_recipient" to ::payoutCreateRecipientReqTransformer,
        "payout_enroll_disburse_account" to ::payoutEnrollDisburseAccountReqTransformer,
        "payout_get" to ::payoutGetReqTransformer,
        "payout_stage" to ::payoutStageReqTransformer,
        "payout_transfer" to ::payoutTransferReqTransformer,
        "payout_void" to ::payoutVoidReqTransformer,
        "post_authenticate" to ::postAuthenticateReqTransformer,
        "pre_authenticate" to ::preAuthenticateReqTransformer,
        "refund" to ::refundReqTransformer,
        "reverse" to ::reverseReqTransformer,
        "setup_recurring" to ::setupRecurringReqTransformer,
        "submit_evidence" to ::submitEvidenceReqTransformer,
        "tokenize" to ::tokenizeReqTransformer,
        "void" to ::voidReqTransformer,
    )

    val resTransformers: Map<String, (ByteArray, ByteArray, ByteArray) -> ByteArray> = mapOf(
        "accept" to ::acceptResTransformer,
        "authenticate" to ::authenticateResTransformer,
        "authorize" to ::authorizeResTransformer,
        "capture" to ::captureResTransformer,
        "charge" to ::chargeResTransformer,
        "create" to ::createResTransformer,
        "create_access_token" to ::createAccessTokenResTransformer,
        "create_order" to ::createOrderResTransformer,
        "create_session_token" to ::createSessionTokenResTransformer,
        "defend" to ::defendResTransformer,
        "get" to ::getResTransformer,
        "payout_create" to ::payoutCreateResTransformer,
        "payout_create_link" to ::payoutCreateLinkResTransformer,
        "payout_create_recipient" to ::payoutCreateRecipientResTransformer,
        "payout_enroll_disburse_account" to ::payoutEnrollDisburseAccountResTransformer,
        "payout_get" to ::payoutGetResTransformer,
        "payout_stage" to ::payoutStageResTransformer,
        "payout_transfer" to ::payoutTransferResTransformer,
        "payout_void" to ::payoutVoidResTransformer,
        "post_authenticate" to ::postAuthenticateResTransformer,
        "pre_authenticate" to ::preAuthenticateResTransformer,
        "refund" to ::refundResTransformer,
        "reverse" to ::reverseResTransformer,
        "setup_recurring" to ::setupRecurringResTransformer,
        "submit_evidence" to ::submitEvidenceResTransformer,
        "tokenize" to ::tokenizeResTransformer,
        "void" to ::voidResTransformer,
    )

    // Single-step flows: direct transformer, no HTTP round-trip.
    val directTransformers: Map<String, (ByteArray, ByteArray) -> ByteArray> = mapOf(
        "handle_event" to ::handleEventTransformer,
    )

}

// Per-service client classes — typed with concrete proto request/response types.

class CustomerClient(
    config: ConnectorConfig,
    defaults: RequestConfig = RequestConfig.getDefaultInstance(),
    libPath: String? = null
) : ConnectorClient(config, defaults, libPath) {
    // create: CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.
    fun create(request: CustomerServiceCreateRequest, options: RequestConfig? = null): CustomerServiceCreateResponse =
        executeFlow("create", request.toByteArray(), CustomerServiceCreateResponse.parser(), options)

}

class DisputeClient(
    config: ConnectorConfig,
    defaults: RequestConfig = RequestConfig.getDefaultInstance(),
    libPath: String? = null
) : ConnectorClient(config, defaults, libPath) {
    // accept: DisputeService.Accept — Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient.
    fun accept(request: DisputeServiceAcceptRequest, options: RequestConfig? = null): DisputeServiceAcceptResponse =
        executeFlow("accept", request.toByteArray(), DisputeServiceAcceptResponse.parser(), options)

    // defend: DisputeService.Defend — Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation.
    fun defend(request: DisputeServiceDefendRequest, options: RequestConfig? = null): DisputeServiceDefendResponse =
        executeFlow("defend", request.toByteArray(), DisputeServiceDefendResponse.parser(), options)

    // submit_evidence: DisputeService.SubmitEvidence — Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims.
    fun submit_evidence(request: DisputeServiceSubmitEvidenceRequest, options: RequestConfig? = null): DisputeServiceSubmitEvidenceResponse =
        executeFlow("submit_evidence", request.toByteArray(), DisputeServiceSubmitEvidenceResponse.parser(), options)

}

class EventClient(
    config: ConnectorConfig,
    defaults: RequestConfig = RequestConfig.getDefaultInstance(),
    libPath: String? = null
) : ConnectorClient(config, defaults, libPath) {
    // handle_event: EventService.HandleEvent — Process webhook notifications from connectors. Translates connector events into standardized responses for asynchronous payment state updates.
    fun handle_event(request: EventServiceHandleRequest, options: RequestConfig? = null): EventServiceHandleResponse =
        executeDirect("handle_event", request.toByteArray(), EventServiceHandleResponse.parser(), options)

}

class MerchantAuthenticationClient(
    config: ConnectorConfig,
    defaults: RequestConfig = RequestConfig.getDefaultInstance(),
    libPath: String? = null
) : ConnectorClient(config, defaults, libPath) {
    // create_access_token: MerchantAuthenticationService.CreateAccessToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.
    fun create_access_token(request: MerchantAuthenticationServiceCreateAccessTokenRequest, options: RequestConfig? = null): MerchantAuthenticationServiceCreateAccessTokenResponse =
        executeFlow("create_access_token", request.toByteArray(), MerchantAuthenticationServiceCreateAccessTokenResponse.parser(), options)

    // create_session_token: MerchantAuthenticationService.CreateSessionToken — Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking.
    fun create_session_token(request: MerchantAuthenticationServiceCreateSessionTokenRequest, options: RequestConfig? = null): MerchantAuthenticationServiceCreateSessionTokenResponse =
        executeFlow("create_session_token", request.toByteArray(), MerchantAuthenticationServiceCreateSessionTokenResponse.parser(), options)

}

class PaymentMethodAuthenticationClient(
    config: ConnectorConfig,
    defaults: RequestConfig = RequestConfig.getDefaultInstance(),
    libPath: String? = null
) : ConnectorClient(config, defaults, libPath) {
    // authenticate: PaymentMethodAuthenticationService.Authenticate — Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.
    fun authenticate(request: PaymentMethodAuthenticationServiceAuthenticateRequest, options: RequestConfig? = null): PaymentMethodAuthenticationServiceAuthenticateResponse =
        executeFlow("authenticate", request.toByteArray(), PaymentMethodAuthenticationServiceAuthenticateResponse.parser(), options)

    // post_authenticate: PaymentMethodAuthenticationService.PostAuthenticate — Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed.
    fun post_authenticate(request: PaymentMethodAuthenticationServicePostAuthenticateRequest, options: RequestConfig? = null): PaymentMethodAuthenticationServicePostAuthenticateResponse =
        executeFlow("post_authenticate", request.toByteArray(), PaymentMethodAuthenticationServicePostAuthenticateResponse.parser(), options)

    // pre_authenticate: PaymentMethodAuthenticationService.PreAuthenticate — Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.
    fun pre_authenticate(request: PaymentMethodAuthenticationServicePreAuthenticateRequest, options: RequestConfig? = null): PaymentMethodAuthenticationServicePreAuthenticateResponse =
        executeFlow("pre_authenticate", request.toByteArray(), PaymentMethodAuthenticationServicePreAuthenticateResponse.parser(), options)

}

class PaymentMethodClient(
    config: ConnectorConfig,
    defaults: RequestConfig = RequestConfig.getDefaultInstance(),
    libPath: String? = null
) : ConnectorClient(config, defaults, libPath) {
    // tokenize: PaymentMethodService.Tokenize — Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.
    fun tokenize(request: PaymentMethodServiceTokenizeRequest, options: RequestConfig? = null): PaymentMethodServiceTokenizeResponse =
        executeFlow("tokenize", request.toByteArray(), PaymentMethodServiceTokenizeResponse.parser(), options)

}

class PaymentClient(
    config: ConnectorConfig,
    defaults: RequestConfig = RequestConfig.getDefaultInstance(),
    libPath: String? = null
) : ConnectorClient(config, defaults, libPath) {
    // authorize: PaymentService.Authorize — Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing.
    fun authorize(request: PaymentServiceAuthorizeRequest, options: RequestConfig? = null): PaymentServiceAuthorizeResponse =
        executeFlow("authorize", request.toByteArray(), PaymentServiceAuthorizeResponse.parser(), options)

    // capture: PaymentService.Capture — Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.
    fun capture(request: PaymentServiceCaptureRequest, options: RequestConfig? = null): PaymentServiceCaptureResponse =
        executeFlow("capture", request.toByteArray(), PaymentServiceCaptureResponse.parser(), options)

    // create_order: PaymentService.CreateOrder — Initialize an order in the payment processor system. Sets up payment context before customer enters card details for improved authorization rates.
    fun create_order(request: PaymentServiceCreateOrderRequest, options: RequestConfig? = null): PaymentServiceCreateOrderResponse =
        executeFlow("create_order", request.toByteArray(), PaymentServiceCreateOrderResponse.parser(), options)

    // get: PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.
    fun get(request: PaymentServiceGetRequest, options: RequestConfig? = null): PaymentServiceGetResponse =
        executeFlow("get", request.toByteArray(), PaymentServiceGetResponse.parser(), options)

    // refund: PaymentService.Refund — Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.
    fun refund(request: PaymentServiceRefundRequest, options: RequestConfig? = null): RefundResponse =
        executeFlow("refund", request.toByteArray(), RefundResponse.parser(), options)

    // reverse: PaymentService.Reverse — Reverse a captured payment before settlement. Recovers funds after capture but before bank settlement, used for corrections or cancellations.
    fun reverse(request: PaymentServiceReverseRequest, options: RequestConfig? = null): PaymentServiceReverseResponse =
        executeFlow("reverse", request.toByteArray(), PaymentServiceReverseResponse.parser(), options)

    // setup_recurring: PaymentService.SetupRecurring — Setup a recurring payment instruction for future payments/ debits. This could be for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.
    fun setup_recurring(request: PaymentServiceSetupRecurringRequest, options: RequestConfig? = null): PaymentServiceSetupRecurringResponse =
        executeFlow("setup_recurring", request.toByteArray(), PaymentServiceSetupRecurringResponse.parser(), options)

    // void: PaymentService.Void — Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.
    fun void(request: PaymentServiceVoidRequest, options: RequestConfig? = null): PaymentServiceVoidResponse =
        executeFlow("void", request.toByteArray(), PaymentServiceVoidResponse.parser(), options)

}

class PayoutClient(
    config: ConnectorConfig,
    defaults: RequestConfig = RequestConfig.getDefaultInstance(),
    libPath: String? = null
) : ConnectorClient(config, defaults, libPath) {
    // payout_create: PayoutService.Create — Creates a payout.
    fun payout_create(request: PayoutServiceCreateRequest, options: RequestConfig? = null): PayoutServiceCreateResponse =
        executeFlow("payout_create", request.toByteArray(), PayoutServiceCreateResponse.parser(), options)

    // payout_create_link: PayoutService.CreateLink — Creates a link between the recipient and the payout.
    fun payout_create_link(request: PayoutServiceCreateLinkRequest, options: RequestConfig? = null): PayoutServiceCreateLinkResponse =
        executeFlow("payout_create_link", request.toByteArray(), PayoutServiceCreateLinkResponse.parser(), options)

    // payout_create_recipient: PayoutService.CreateRecipient — Create payout recipient.
    fun payout_create_recipient(request: PayoutServiceCreateRecipientRequest, options: RequestConfig? = null): PayoutServiceCreateRecipientResponse =
        executeFlow("payout_create_recipient", request.toByteArray(), PayoutServiceCreateRecipientResponse.parser(), options)

    // payout_enroll_disburse_account: PayoutService.EnrollDisburseAccount — Enroll disburse account.
    fun payout_enroll_disburse_account(request: PayoutServiceEnrollDisburseAccountRequest, options: RequestConfig? = null): PayoutServiceEnrollDisburseAccountResponse =
        executeFlow("payout_enroll_disburse_account", request.toByteArray(), PayoutServiceEnrollDisburseAccountResponse.parser(), options)

    // payout_get: PayoutService.Get — Retrieve payout details.
    fun payout_get(request: PayoutServiceGetRequest, options: RequestConfig? = null): PayoutServiceGetResponse =
        executeFlow("payout_get", request.toByteArray(), PayoutServiceGetResponse.parser(), options)

    // payout_stage: PayoutService.Stage — Stage the payout.
    fun payout_stage(request: PayoutServiceStageRequest, options: RequestConfig? = null): PayoutServiceStageResponse =
        executeFlow("payout_stage", request.toByteArray(), PayoutServiceStageResponse.parser(), options)

    // payout_transfer: PayoutService.Transfer — Creates a payout fund transfer.
    fun payout_transfer(request: PayoutServiceTransferRequest, options: RequestConfig? = null): PayoutServiceTransferResponse =
        executeFlow("payout_transfer", request.toByteArray(), PayoutServiceTransferResponse.parser(), options)

    // payout_void: PayoutService.Void — Void a payout.
    fun payout_void(request: PayoutServiceVoidRequest, options: RequestConfig? = null): PayoutServiceVoidResponse =
        executeFlow("payout_void", request.toByteArray(), PayoutServiceVoidResponse.parser(), options)

}

class RecurringPaymentClient(
    config: ConnectorConfig,
    defaults: RequestConfig = RequestConfig.getDefaultInstance(),
    libPath: String? = null
) : ConnectorClient(config, defaults, libPath) {
    // charge: RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.
    fun charge(request: RecurringPaymentServiceChargeRequest, options: RequestConfig? = null): RecurringPaymentServiceChargeResponse =
        executeFlow("charge", request.toByteArray(), RecurringPaymentServiceChargeResponse.parser(), options)

}
