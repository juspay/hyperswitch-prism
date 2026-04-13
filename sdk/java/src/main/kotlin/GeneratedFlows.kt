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
import uniffi.connector_service_ffi.createClientAuthenticationTokenReqTransformer
import uniffi.connector_service_ffi.createClientAuthenticationTokenResTransformer
import uniffi.connector_service_ffi.createOrderReqTransformer
import uniffi.connector_service_ffi.createOrderResTransformer
import uniffi.connector_service_ffi.createServerAuthenticationTokenReqTransformer
import uniffi.connector_service_ffi.createServerAuthenticationTokenResTransformer
import uniffi.connector_service_ffi.createServerSessionAuthenticationTokenReqTransformer
import uniffi.connector_service_ffi.createServerSessionAuthenticationTokenResTransformer
import uniffi.connector_service_ffi.defendReqTransformer
import uniffi.connector_service_ffi.defendResTransformer
import uniffi.connector_service_ffi.getReqTransformer
import uniffi.connector_service_ffi.getResTransformer
import uniffi.connector_service_ffi.incrementalAuthorizationReqTransformer
import uniffi.connector_service_ffi.incrementalAuthorizationResTransformer
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
import uniffi.connector_service_ffi.proxyAuthorizeReqTransformer
import uniffi.connector_service_ffi.proxyAuthorizeResTransformer
import uniffi.connector_service_ffi.proxySetupRecurringReqTransformer
import uniffi.connector_service_ffi.proxySetupRecurringResTransformer
import uniffi.connector_service_ffi.recurringRevokeReqTransformer
import uniffi.connector_service_ffi.recurringRevokeResTransformer
import uniffi.connector_service_ffi.refundReqTransformer
import uniffi.connector_service_ffi.refundResTransformer
import uniffi.connector_service_ffi.refundGetReqTransformer
import uniffi.connector_service_ffi.refundGetResTransformer
import uniffi.connector_service_ffi.reverseReqTransformer
import uniffi.connector_service_ffi.reverseResTransformer
import uniffi.connector_service_ffi.setupRecurringReqTransformer
import uniffi.connector_service_ffi.setupRecurringResTransformer
import uniffi.connector_service_ffi.submitEvidenceReqTransformer
import uniffi.connector_service_ffi.submitEvidenceResTransformer
import uniffi.connector_service_ffi.tokenAuthorizeReqTransformer
import uniffi.connector_service_ffi.tokenAuthorizeResTransformer
import uniffi.connector_service_ffi.tokenSetupRecurringReqTransformer
import uniffi.connector_service_ffi.tokenSetupRecurringResTransformer
import uniffi.connector_service_ffi.tokenizeReqTransformer
import uniffi.connector_service_ffi.tokenizeResTransformer
import uniffi.connector_service_ffi.voidReqTransformer
import uniffi.connector_service_ffi.voidResTransformer
import uniffi.connector_service_ffi.handleEventTransformer
import uniffi.connector_service_ffi.verifyRedirectResponseTransformer

object FlowRegistry {
    val reqTransformers: Map<String, (ByteArray, ByteArray) -> ByteArray> = mapOf(
        "accept" to ::acceptReqTransformer,
        "authenticate" to ::authenticateReqTransformer,
        "authorize" to ::authorizeReqTransformer,
        "capture" to ::captureReqTransformer,
        "charge" to ::chargeReqTransformer,
        "create" to ::createReqTransformer,
        "create_client_authentication_token" to ::createClientAuthenticationTokenReqTransformer,
        "create_order" to ::createOrderReqTransformer,
        "create_server_authentication_token" to ::createServerAuthenticationTokenReqTransformer,
        "create_server_session_authentication_token" to ::createServerSessionAuthenticationTokenReqTransformer,
        "defend" to ::defendReqTransformer,
        "get" to ::getReqTransformer,
        "incremental_authorization" to ::incrementalAuthorizationReqTransformer,
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
        "proxy_authorize" to ::proxyAuthorizeReqTransformer,
        "proxy_setup_recurring" to ::proxySetupRecurringReqTransformer,
        "recurring_revoke" to ::recurringRevokeReqTransformer,
        "refund" to ::refundReqTransformer,
        "refund_get" to ::refundGetReqTransformer,
        "reverse" to ::reverseReqTransformer,
        "setup_recurring" to ::setupRecurringReqTransformer,
        "submit_evidence" to ::submitEvidenceReqTransformer,
        "token_authorize" to ::tokenAuthorizeReqTransformer,
        "token_setup_recurring" to ::tokenSetupRecurringReqTransformer,
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
        "create_client_authentication_token" to ::createClientAuthenticationTokenResTransformer,
        "create_order" to ::createOrderResTransformer,
        "create_server_authentication_token" to ::createServerAuthenticationTokenResTransformer,
        "create_server_session_authentication_token" to ::createServerSessionAuthenticationTokenResTransformer,
        "defend" to ::defendResTransformer,
        "get" to ::getResTransformer,
        "incremental_authorization" to ::incrementalAuthorizationResTransformer,
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
        "proxy_authorize" to ::proxyAuthorizeResTransformer,
        "proxy_setup_recurring" to ::proxySetupRecurringResTransformer,
        "recurring_revoke" to ::recurringRevokeResTransformer,
        "refund" to ::refundResTransformer,
        "refund_get" to ::refundGetResTransformer,
        "reverse" to ::reverseResTransformer,
        "setup_recurring" to ::setupRecurringResTransformer,
        "submit_evidence" to ::submitEvidenceResTransformer,
        "token_authorize" to ::tokenAuthorizeResTransformer,
        "token_setup_recurring" to ::tokenSetupRecurringResTransformer,
        "tokenize" to ::tokenizeResTransformer,
        "void" to ::voidResTransformer,
    )

    // Single-step flows: direct transformer, no HTTP round-trip.
    val directTransformers: Map<String, (ByteArray, ByteArray) -> ByteArray> = mapOf(
        "handle_event" to ::handleEventTransformer,
        "verify_redirect_response" to ::verifyRedirectResponseTransformer,
    )

}

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
    // create_client_authentication_token: MerchantAuthenticationService.CreateClientAuthenticationToken — Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.
    fun create_client_authentication_token(request: MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest, options: RequestConfig? = null): MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse =
        executeFlow("create_client_authentication_token", request.toByteArray(), MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse.parser(), options)

    // create_server_authentication_token: MerchantAuthenticationService.CreateServerAuthenticationToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.
    fun create_server_authentication_token(request: MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest, options: RequestConfig? = null): MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse =
        executeFlow("create_server_authentication_token", request.toByteArray(), MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse.parser(), options)

    // create_server_session_authentication_token: MerchantAuthenticationService.CreateServerSessionAuthenticationToken — Create a server-side session with the connector. Establishes session state for multi-step operations like 3DS verification or wallet authorization.
    fun create_server_session_authentication_token(request: MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest, options: RequestConfig? = null): MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse =
        executeFlow("create_server_session_authentication_token", request.toByteArray(), MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse.parser(), options)

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

    // capture: PaymentService.Capture — Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.
    fun capture(request: PaymentServiceCaptureRequest, options: RequestConfig? = null): PaymentServiceCaptureResponse =
        executeFlow("capture", request.toByteArray(), PaymentServiceCaptureResponse.parser(), options)

    // create_order: PaymentService.CreateOrder — Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.
    fun create_order(request: PaymentServiceCreateOrderRequest, options: RequestConfig? = null): PaymentServiceCreateOrderResponse =
        executeFlow("create_order", request.toByteArray(), PaymentServiceCreateOrderResponse.parser(), options)

    // get: PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.
    fun get(request: PaymentServiceGetRequest, options: RequestConfig? = null): PaymentServiceGetResponse =
        executeFlow("get", request.toByteArray(), PaymentServiceGetResponse.parser(), options)

    // incremental_authorization: PaymentService.IncrementalAuthorization — Increase the authorized amount for an existing payment. Enables you to capture additional funds when the transaction amount changes after initial authorization.
    fun incremental_authorization(request: PaymentServiceIncrementalAuthorizationRequest, options: RequestConfig? = null): PaymentServiceIncrementalAuthorizationResponse =
        executeFlow("incremental_authorization", request.toByteArray(), PaymentServiceIncrementalAuthorizationResponse.parser(), options)

    // proxy_authorize: PaymentService.ProxyAuthorize — Authorize using vault-aliased card data. Proxy substitutes before connector.
    fun proxy_authorize(request: PaymentServiceProxyAuthorizeRequest, options: RequestConfig? = null): PaymentServiceAuthorizeResponse =
        executeFlow("proxy_authorize", request.toByteArray(), PaymentServiceAuthorizeResponse.parser(), options)

    // proxy_setup_recurring: PaymentService.ProxySetupRecurring — Setup recurring mandate using vault-aliased card data.
    fun proxy_setup_recurring(request: PaymentServiceProxySetupRecurringRequest, options: RequestConfig? = null): PaymentServiceSetupRecurringResponse =
        executeFlow("proxy_setup_recurring", request.toByteArray(), PaymentServiceSetupRecurringResponse.parser(), options)

    // refund: PaymentService.Refund — Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.
    fun refund(request: PaymentServiceRefundRequest, options: RequestConfig? = null): RefundResponse =
        executeFlow("refund", request.toByteArray(), RefundResponse.parser(), options)

    // reverse: PaymentService.Reverse — Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.
    fun reverse(request: PaymentServiceReverseRequest, options: RequestConfig? = null): PaymentServiceReverseResponse =
        executeFlow("reverse", request.toByteArray(), PaymentServiceReverseResponse.parser(), options)

    // setup_recurring: PaymentService.SetupRecurring — Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.
    fun setup_recurring(request: PaymentServiceSetupRecurringRequest, options: RequestConfig? = null): PaymentServiceSetupRecurringResponse =
        executeFlow("setup_recurring", request.toByteArray(), PaymentServiceSetupRecurringResponse.parser(), options)

    // token_authorize: PaymentService.TokenAuthorize — Authorize using a connector-issued payment method token.
    fun token_authorize(request: PaymentServiceTokenAuthorizeRequest, options: RequestConfig? = null): PaymentServiceAuthorizeResponse =
        executeFlow("token_authorize", request.toByteArray(), PaymentServiceAuthorizeResponse.parser(), options)

    // token_setup_recurring: PaymentService.TokenSetupRecurring — Setup a recurring mandate using a connector token.
    fun token_setup_recurring(request: PaymentServiceTokenSetupRecurringRequest, options: RequestConfig? = null): PaymentServiceSetupRecurringResponse =
        executeFlow("token_setup_recurring", request.toByteArray(), PaymentServiceSetupRecurringResponse.parser(), options)

    // void: PaymentService.Void — Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.
    fun void(request: PaymentServiceVoidRequest, options: RequestConfig? = null): PaymentServiceVoidResponse =
        executeFlow("void", request.toByteArray(), PaymentServiceVoidResponse.parser(), options)

    // verify_redirect_response: PaymentService.VerifyRedirectResponse — Verify and process redirect responses from 3D Secure or other external flows. Validates authentication results and updates payment state accordingly.
    fun verify_redirect_response(request: PaymentServiceVerifyRedirectResponseRequest, options: RequestConfig? = null): PaymentServiceVerifyRedirectResponseResponse =
        executeDirect("verify_redirect_response", request.toByteArray(), PaymentServiceVerifyRedirectResponseResponse.parser(), options)

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

    // recurring_revoke: RecurringPaymentService.Revoke — Cancel an existing recurring payment mandate. Stops future automatic charges on customer's stored consent for subscription cancellations.
    fun recurring_revoke(request: RecurringPaymentServiceRevokeRequest, options: RequestConfig? = null): RecurringPaymentServiceRevokeResponse =
        executeFlow("recurring_revoke", request.toByteArray(), RecurringPaymentServiceRevokeResponse.parser(), options)

}

class RefundClient(
    config: ConnectorConfig,
    defaults: RequestConfig = RequestConfig.getDefaultInstance(),
    libPath: String? = null
) : ConnectorClient(config, defaults, libPath) {
    // refund_get: RefundService.Get — Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.
    fun refund_get(request: RefundServiceGetRequest, options: RequestConfig? = null): RefundResponse =
        executeFlow("refund_get", request.toByteArray(), RefundResponse.parser(), options)

}
