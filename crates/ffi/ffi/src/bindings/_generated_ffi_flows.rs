// AUTO-GENERATED — do not edit by hand.
// Source: services.proto ∩ services/*.rs  |  Regenerate: make generate

use grpc_api_types::payments::{
    CustomerServiceCreateRequest,
    DisputeServiceAcceptRequest,
    DisputeServiceDefendRequest,
    DisputeServiceSubmitEvidenceRequest,
    MerchantAuthenticationServiceCreateAccessTokenRequest,
    MerchantAuthenticationServiceCreateSessionTokenRequest,
    PaymentMethodAuthenticationServiceAuthenticateRequest,
    PaymentMethodAuthenticationServicePostAuthenticateRequest,
    PaymentMethodAuthenticationServicePreAuthenticateRequest,
    PaymentMethodServiceTokenizeRequest,
    PaymentServiceAuthorizeRequest,
    PaymentServiceCaptureRequest,
    PaymentServiceCreateOrderRequest,
    PaymentServiceGetRequest,
    PaymentServiceRefundRequest,
    PaymentServiceReverseRequest,
    PaymentServiceSetupRecurringRequest,
    PaymentServiceVoidRequest,
    ProxiedPaymentServiceAuthorizeRequest,
    ProxiedPaymentServiceSetupRecurringRequest,
    RecurringPaymentServiceChargeRequest,
    TokenizedPaymentServiceAuthorizeRequest,
    TokenizedPaymentServiceSetupRecurringRequest,
};
use grpc_api_types::payouts::{
    PayoutServiceCreateLinkRequest,
    PayoutServiceCreateRecipientRequest,
    PayoutServiceCreateRequest,
    PayoutServiceEnrollDisburseAccountRequest,
    PayoutServiceGetRequest,
    PayoutServiceStageRequest,
    PayoutServiceTransferRequest,
    PayoutServiceVoidRequest,
};

use crate::handlers::payments::{
    accept_req_handler, accept_res_handler,
    authenticate_req_handler, authenticate_res_handler,
    authorize_req_handler, authorize_res_handler,
    capture_req_handler, capture_res_handler,
    charge_req_handler, charge_res_handler,
    create_req_handler, create_res_handler,
    create_access_token_req_handler, create_access_token_res_handler,
    create_order_req_handler, create_order_res_handler,
    create_session_token_req_handler, create_session_token_res_handler,
    defend_req_handler, defend_res_handler,
    get_req_handler, get_res_handler,
    payout_create_req_handler, payout_create_res_handler,
    payout_create_link_req_handler, payout_create_link_res_handler,
    payout_create_recipient_req_handler, payout_create_recipient_res_handler,
    payout_enroll_disburse_account_req_handler, payout_enroll_disburse_account_res_handler,
    payout_get_req_handler, payout_get_res_handler,
    payout_stage_req_handler, payout_stage_res_handler,
    payout_transfer_req_handler, payout_transfer_res_handler,
    payout_void_req_handler, payout_void_res_handler,
    post_authenticate_req_handler, post_authenticate_res_handler,
    pre_authenticate_req_handler, pre_authenticate_res_handler,
    proxied_authorize_req_handler, proxied_authorize_res_handler,
    proxied_setup_recurring_req_handler, proxied_setup_recurring_res_handler,
    refund_req_handler, refund_res_handler,
    reverse_req_handler, reverse_res_handler,
    setup_recurring_req_handler, setup_recurring_res_handler,
    submit_evidence_req_handler, submit_evidence_res_handler,
    tokenize_req_handler, tokenize_res_handler,
    tokenized_authorize_req_handler, tokenized_authorize_res_handler,
    tokenized_setup_recurring_req_handler, tokenized_setup_recurring_res_handler,
    void_req_handler, void_res_handler,
};

// accept: DisputeService.Accept — Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient.
define_ffi_flow!(accept, DisputeServiceAcceptRequest, accept_req_handler, accept_res_handler);
// authenticate: PaymentMethodAuthenticationService.Authenticate — Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.
define_ffi_flow!(authenticate, PaymentMethodAuthenticationServiceAuthenticateRequest, authenticate_req_handler, authenticate_res_handler);
// authorize: DirectPaymentService.Authorize — Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing.
define_ffi_flow!(authorize, PaymentServiceAuthorizeRequest, authorize_req_handler, authorize_res_handler);
// capture: DirectPaymentService.Capture — Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.
define_ffi_flow!(capture, PaymentServiceCaptureRequest, capture_req_handler, capture_res_handler);
// charge: RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.
define_ffi_flow!(charge, RecurringPaymentServiceChargeRequest, charge_req_handler, charge_res_handler);
// create: CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.
define_ffi_flow!(create, CustomerServiceCreateRequest, create_req_handler, create_res_handler);
// create_access_token: MerchantAuthenticationService.CreateAccessToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.
define_ffi_flow!(create_access_token, MerchantAuthenticationServiceCreateAccessTokenRequest, create_access_token_req_handler, create_access_token_res_handler);
// create_order: DirectPaymentService.CreateOrder — Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.
define_ffi_flow!(create_order, PaymentServiceCreateOrderRequest, create_order_req_handler, create_order_res_handler);
// create_session_token: MerchantAuthenticationService.CreateSessionToken — Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking.
define_ffi_flow!(create_session_token, MerchantAuthenticationServiceCreateSessionTokenRequest, create_session_token_req_handler, create_session_token_res_handler);
// defend: DisputeService.Defend — Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation.
define_ffi_flow!(defend, DisputeServiceDefendRequest, defend_req_handler, defend_res_handler);
// get: DirectPaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.
define_ffi_flow!(get, PaymentServiceGetRequest, get_req_handler, get_res_handler);
// payout_create: PayoutService.Create — Creates a payout.
define_ffi_flow!(payout_create, PayoutServiceCreateRequest, payout_create_req_handler, payout_create_res_handler);
// payout_create_link: PayoutService.CreateLink — Creates a link between the recipient and the payout.
define_ffi_flow!(payout_create_link, PayoutServiceCreateLinkRequest, payout_create_link_req_handler, payout_create_link_res_handler);
// payout_create_recipient: PayoutService.CreateRecipient — Create payout recipient.
define_ffi_flow!(payout_create_recipient, PayoutServiceCreateRecipientRequest, payout_create_recipient_req_handler, payout_create_recipient_res_handler);
// payout_enroll_disburse_account: PayoutService.EnrollDisburseAccount — Enroll disburse account.
define_ffi_flow!(payout_enroll_disburse_account, PayoutServiceEnrollDisburseAccountRequest, payout_enroll_disburse_account_req_handler, payout_enroll_disburse_account_res_handler);
// payout_get: PayoutService.Get — Retrieve payout details.
define_ffi_flow!(payout_get, PayoutServiceGetRequest, payout_get_req_handler, payout_get_res_handler);
// payout_stage: PayoutService.Stage — Stage the payout.
define_ffi_flow!(payout_stage, PayoutServiceStageRequest, payout_stage_req_handler, payout_stage_res_handler);
// payout_transfer: PayoutService.Transfer — Creates a payout fund transfer.
define_ffi_flow!(payout_transfer, PayoutServiceTransferRequest, payout_transfer_req_handler, payout_transfer_res_handler);
// payout_void: PayoutService.Void — Void a payout.
define_ffi_flow!(payout_void, PayoutServiceVoidRequest, payout_void_req_handler, payout_void_res_handler);
// post_authenticate: PaymentMethodAuthenticationService.PostAuthenticate — Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed.
define_ffi_flow!(post_authenticate, PaymentMethodAuthenticationServicePostAuthenticateRequest, post_authenticate_req_handler, post_authenticate_res_handler);
// pre_authenticate: PaymentMethodAuthenticationService.PreAuthenticate — Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.
define_ffi_flow!(pre_authenticate, PaymentMethodAuthenticationServicePreAuthenticateRequest, pre_authenticate_req_handler, pre_authenticate_res_handler);
// proxied_authorize: ProxiedPaymentService.Authorize — Authorize using vault-aliased card data. Proxy substitutes before connector.
define_ffi_flow!(proxied_authorize, ProxiedPaymentServiceAuthorizeRequest, proxied_authorize_req_handler, proxied_authorize_res_handler);
// proxied_setup_recurring: ProxiedPaymentService.SetupRecurring — Setup recurring mandate using vault-aliased card data.
define_ffi_flow!(proxied_setup_recurring, ProxiedPaymentServiceSetupRecurringRequest, proxied_setup_recurring_req_handler, proxied_setup_recurring_res_handler);
// refund: DirectPaymentService.Refund — Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.
define_ffi_flow!(refund, PaymentServiceRefundRequest, refund_req_handler, refund_res_handler);
// reverse: DirectPaymentService.Reverse — Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.
define_ffi_flow!(reverse, PaymentServiceReverseRequest, reverse_req_handler, reverse_res_handler);
// setup_recurring: DirectPaymentService.SetupRecurring — Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.
define_ffi_flow!(setup_recurring, PaymentServiceSetupRecurringRequest, setup_recurring_req_handler, setup_recurring_res_handler);
// submit_evidence: DisputeService.SubmitEvidence — Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims.
define_ffi_flow!(submit_evidence, DisputeServiceSubmitEvidenceRequest, submit_evidence_req_handler, submit_evidence_res_handler);
// tokenize: PaymentMethodService.Tokenize — Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.
define_ffi_flow!(tokenize, PaymentMethodServiceTokenizeRequest, tokenize_req_handler, tokenize_res_handler);
// tokenized_authorize: TokenizedPaymentService.Authorize — Authorize using a connector-issued payment method token.
define_ffi_flow!(tokenized_authorize, TokenizedPaymentServiceAuthorizeRequest, tokenized_authorize_req_handler, tokenized_authorize_res_handler);
// tokenized_setup_recurring: TokenizedPaymentService.SetupRecurring — Setup a recurring mandate using a connector token.
define_ffi_flow!(tokenized_setup_recurring, TokenizedPaymentServiceSetupRecurringRequest, tokenized_setup_recurring_req_handler, tokenized_setup_recurring_res_handler);
// void: DirectPaymentService.Void — Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.
define_ffi_flow!(void, PaymentServiceVoidRequest, void_req_handler, void_res_handler);
