// AUTO-GENERATED — do not edit by hand.
// Source: services.proto ∩ services/*.rs  |  Regenerate: make generate

use grpc_api_types::payments::{
    CustomerServiceCreateRequest,
    CustomerServiceCreateResponse,
    DisputeServiceAcceptRequest,
    DisputeServiceAcceptResponse,
    DisputeServiceDefendRequest,
    DisputeServiceDefendResponse,
    DisputeServiceSubmitEvidenceRequest,
    DisputeServiceSubmitEvidenceResponse,
    MerchantAuthenticationServiceCreateAccessTokenRequest,
    MerchantAuthenticationServiceCreateAccessTokenResponse,
    MerchantAuthenticationServiceCreateSessionTokenRequest,
    MerchantAuthenticationServiceCreateSessionTokenResponse,
    PaymentMethodAuthenticationServiceAuthenticateRequest,
    PaymentMethodAuthenticationServiceAuthenticateResponse,
    PaymentMethodAuthenticationServicePostAuthenticateRequest,
    PaymentMethodAuthenticationServicePostAuthenticateResponse,
    PaymentMethodAuthenticationServicePreAuthenticateRequest,
    PaymentMethodAuthenticationServicePreAuthenticateResponse,
    PaymentMethodServiceTokenizeRequest,
    PaymentMethodServiceTokenizeResponse,
    PaymentServiceAuthorizeRequest,
    PaymentServiceAuthorizeResponse,
    PaymentServiceCaptureRequest,
    PaymentServiceCaptureResponse,
    PaymentServiceCreateOrderRequest,
    PaymentServiceCreateOrderResponse,
    PaymentServiceGetRequest,
    PaymentServiceGetResponse,
    PaymentServiceRefundRequest,
    PaymentServiceReverseRequest,
    PaymentServiceReverseResponse,
    PaymentServiceSetupRecurringRequest,
    PaymentServiceSetupRecurringResponse,
    PaymentServiceVoidRequest,
    PaymentServiceVoidResponse,
    RecurringPaymentServiceChargeRequest,
    RecurringPaymentServiceChargeResponse,
    RefundResponse,
};
use grpc_api_types::payouts::{
    PayoutServiceCreateLinkRequest,
    PayoutServiceCreateLinkResponse,
    PayoutServiceCreateRecipientRequest,
    PayoutServiceCreateRecipientResponse,
    PayoutServiceCreateRequest,
    PayoutServiceCreateResponse,
    PayoutServiceEnrollDisburseAccountRequest,
    PayoutServiceEnrollDisburseAccountResponse,
    PayoutServiceGetRequest,
    PayoutServiceGetResponse,
    PayoutServiceStageRequest,
    PayoutServiceStageResponse,
    PayoutServiceTransferRequest,
    PayoutServiceTransferResponse,
    PayoutServiceVoidRequest,
    PayoutServiceVoidResponse,
};

use crate::services::payments::{
    accept_req_transformer, accept_res_transformer,
    authenticate_req_transformer, authenticate_res_transformer,
    authorize_req_transformer, authorize_res_transformer,
    capture_req_transformer, capture_res_transformer,
    charge_req_transformer, charge_res_transformer,
    create_req_transformer, create_res_transformer,
    create_access_token_req_transformer, create_access_token_res_transformer,
    create_order_req_transformer, create_order_res_transformer,
    create_session_token_req_transformer, create_session_token_res_transformer,
    defend_req_transformer, defend_res_transformer,
    get_req_transformer, get_res_transformer,
    post_authenticate_req_transformer, post_authenticate_res_transformer,
    pre_authenticate_req_transformer, pre_authenticate_res_transformer,
    refund_req_transformer, refund_res_transformer,
    reverse_req_transformer, reverse_res_transformer,
    setup_recurring_req_transformer, setup_recurring_res_transformer,
    submit_evidence_req_transformer, submit_evidence_res_transformer,
    tokenize_req_transformer, tokenize_res_transformer,
    void_req_transformer, void_res_transformer,
};
use crate::services::payouts::{
    payout_create_req_transformer, payout_create_res_transformer,
    payout_create_link_req_transformer, payout_create_link_res_transformer,
    payout_create_recipient_req_transformer, payout_create_recipient_res_transformer,
    payout_enroll_disburse_account_req_transformer, payout_enroll_disburse_account_res_transformer,
    payout_get_req_transformer, payout_get_res_transformer,
    payout_stage_req_transformer, payout_stage_res_transformer,
    payout_transfer_req_transformer, payout_transfer_res_transformer,
    payout_void_req_transformer, payout_void_res_transformer,
};

// accept: DisputeService.Accept — Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient.
impl_flow_handlers!(accept, DisputeServiceAcceptRequest, DisputeServiceAcceptResponse, accept_req_transformer, accept_res_transformer);
// authenticate: PaymentMethodAuthenticationService.Authenticate — Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.
impl_flow_handlers!(authenticate, PaymentMethodAuthenticationServiceAuthenticateRequest, PaymentMethodAuthenticationServiceAuthenticateResponse, authenticate_req_transformer, authenticate_res_transformer);
// authorize: PaymentService.Authorize — Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing.
impl_flow_handlers!(authorize, PaymentServiceAuthorizeRequest, PaymentServiceAuthorizeResponse, authorize_req_transformer, authorize_res_transformer);
// capture: PaymentService.Capture — Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.
impl_flow_handlers!(capture, PaymentServiceCaptureRequest, PaymentServiceCaptureResponse, capture_req_transformer, capture_res_transformer);
// charge: RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.
impl_flow_handlers!(charge, RecurringPaymentServiceChargeRequest, RecurringPaymentServiceChargeResponse, charge_req_transformer, charge_res_transformer);
// create: CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.
impl_flow_handlers!(create, CustomerServiceCreateRequest, CustomerServiceCreateResponse, create_req_transformer, create_res_transformer);
// create_access_token: MerchantAuthenticationService.CreateAccessToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.
impl_flow_handlers!(create_access_token, MerchantAuthenticationServiceCreateAccessTokenRequest, MerchantAuthenticationServiceCreateAccessTokenResponse, create_access_token_req_transformer, create_access_token_res_transformer);
// create_order: PaymentService.CreateOrder — Initialize an order in the payment processor system. Sets up payment context before customer enters card details for improved authorization rates.
impl_flow_handlers!(create_order, PaymentServiceCreateOrderRequest, PaymentServiceCreateOrderResponse, create_order_req_transformer, create_order_res_transformer);
// create_session_token: MerchantAuthenticationService.CreateSessionToken — Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking.
impl_flow_handlers!(create_session_token, MerchantAuthenticationServiceCreateSessionTokenRequest, MerchantAuthenticationServiceCreateSessionTokenResponse, create_session_token_req_transformer, create_session_token_res_transformer);
// defend: DisputeService.Defend — Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation.
impl_flow_handlers!(defend, DisputeServiceDefendRequest, DisputeServiceDefendResponse, defend_req_transformer, defend_res_transformer);
// get: PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.
impl_flow_handlers!(get, PaymentServiceGetRequest, PaymentServiceGetResponse, get_req_transformer, get_res_transformer);
// payout_create: PayoutService.Create — Creates a payout.
impl_flow_handlers!(payout_create, PayoutServiceCreateRequest, PayoutServiceCreateResponse, payout_create_req_transformer, payout_create_res_transformer);
// payout_create_link: PayoutService.CreateLink — Creates a link between the recipient and the payout.
impl_flow_handlers!(payout_create_link, PayoutServiceCreateLinkRequest, PayoutServiceCreateLinkResponse, payout_create_link_req_transformer, payout_create_link_res_transformer);
// payout_create_recipient: PayoutService.CreateRecipient — Create payout recipient.
impl_flow_handlers!(payout_create_recipient, PayoutServiceCreateRecipientRequest, PayoutServiceCreateRecipientResponse, payout_create_recipient_req_transformer, payout_create_recipient_res_transformer);
// payout_enroll_disburse_account: PayoutService.EnrollDisburseAccount — Enroll disburse account.
impl_flow_handlers!(payout_enroll_disburse_account, PayoutServiceEnrollDisburseAccountRequest, PayoutServiceEnrollDisburseAccountResponse, payout_enroll_disburse_account_req_transformer, payout_enroll_disburse_account_res_transformer);
// payout_get: PayoutService.Get — Retrieve payout details.
impl_flow_handlers!(payout_get, PayoutServiceGetRequest, PayoutServiceGetResponse, payout_get_req_transformer, payout_get_res_transformer);
// payout_stage: PayoutService.Stage — Stage the payout.
impl_flow_handlers!(payout_stage, PayoutServiceStageRequest, PayoutServiceStageResponse, payout_stage_req_transformer, payout_stage_res_transformer);
// payout_transfer: PayoutService.Transfer — Creates a payout fund transfer.
impl_flow_handlers!(payout_transfer, PayoutServiceTransferRequest, PayoutServiceTransferResponse, payout_transfer_req_transformer, payout_transfer_res_transformer);
// payout_void: PayoutService.Void — Void a payout.
impl_flow_handlers!(payout_void, PayoutServiceVoidRequest, PayoutServiceVoidResponse, payout_void_req_transformer, payout_void_res_transformer);
// post_authenticate: PaymentMethodAuthenticationService.PostAuthenticate — Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed.
impl_flow_handlers!(post_authenticate, PaymentMethodAuthenticationServicePostAuthenticateRequest, PaymentMethodAuthenticationServicePostAuthenticateResponse, post_authenticate_req_transformer, post_authenticate_res_transformer);
// pre_authenticate: PaymentMethodAuthenticationService.PreAuthenticate — Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.
impl_flow_handlers!(pre_authenticate, PaymentMethodAuthenticationServicePreAuthenticateRequest, PaymentMethodAuthenticationServicePreAuthenticateResponse, pre_authenticate_req_transformer, pre_authenticate_res_transformer);
// refund: PaymentService.Refund — Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.
impl_flow_handlers!(refund, PaymentServiceRefundRequest, RefundResponse, refund_req_transformer, refund_res_transformer);
// reverse: PaymentService.Reverse — Reverse a captured payment before settlement. Recovers funds after capture but before bank settlement, used for corrections or cancellations.
impl_flow_handlers!(reverse, PaymentServiceReverseRequest, PaymentServiceReverseResponse, reverse_req_transformer, reverse_res_transformer);
// setup_recurring: PaymentService.SetupRecurring — Setup a recurring payment instruction for future payments/ debits. This could be for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.
impl_flow_handlers!(setup_recurring, PaymentServiceSetupRecurringRequest, PaymentServiceSetupRecurringResponse, setup_recurring_req_transformer, setup_recurring_res_transformer);
// submit_evidence: DisputeService.SubmitEvidence — Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims.
impl_flow_handlers!(submit_evidence, DisputeServiceSubmitEvidenceRequest, DisputeServiceSubmitEvidenceResponse, submit_evidence_req_transformer, submit_evidence_res_transformer);
// tokenize: PaymentMethodService.Tokenize — Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.
impl_flow_handlers!(tokenize, PaymentMethodServiceTokenizeRequest, PaymentMethodServiceTokenizeResponse, tokenize_req_transformer, tokenize_res_transformer);
// void: PaymentService.Void — Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.
impl_flow_handlers!(void, PaymentServiceVoidRequest, PaymentServiceVoidResponse, void_req_transformer, void_res_transformer);
