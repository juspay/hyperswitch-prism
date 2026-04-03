// AUTO-GENERATED — do not edit by hand.
// Source: services.proto ∩ bindings/uniffi.rs  |  Regenerate: make generate
"use strict";

const FLOWS = {
  // accept: DisputeService.Accept — Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient.
  accept                                     : { request: "DisputeServiceAcceptRequest", response: "DisputeServiceAcceptResponse" },

  // authenticate: PaymentMethodAuthenticationService.Authenticate — Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.
  authenticate                               : { request: "PaymentMethodAuthenticationServiceAuthenticateRequest", response: "PaymentMethodAuthenticationServiceAuthenticateResponse" },

  // authorize: PaymentService.Authorize — Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing.
  authorize                                  : { request: "PaymentServiceAuthorizeRequest", response: "PaymentServiceAuthorizeResponse" },

  // capture: PaymentService.Capture — Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.
  capture                                    : { request: "PaymentServiceCaptureRequest", response: "PaymentServiceCaptureResponse" },

  // charge: RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.
  charge                                     : { request: "RecurringPaymentServiceChargeRequest", response: "RecurringPaymentServiceChargeResponse" },

  // create: CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.
  create                                     : { request: "CustomerServiceCreateRequest", response: "CustomerServiceCreateResponse" },

  // create_client_authentication_token: MerchantAuthenticationService.CreateClientAuthenticationToken — Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.
  create_client_authentication_token         : { request: "MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest", response: "MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse" },

  // create_order: PaymentService.CreateOrder — Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.
  create_order                               : { request: "PaymentServiceCreateOrderRequest", response: "PaymentServiceCreateOrderResponse" },

  // create_server_authentication_token: MerchantAuthenticationService.CreateServerAuthenticationToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.
  create_server_authentication_token         : { request: "MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest", response: "MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse" },

  // create_server_session_authentication_token: MerchantAuthenticationService.CreateServerSessionAuthenticationToken — Create a server-side session with the connector. Establishes session state for multi-step operations like 3DS verification or wallet authorization.
  create_server_session_authentication_token : { request: "MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest", response: "MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse" },

  // defend: DisputeService.Defend — Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation.
  defend                                     : { request: "DisputeServiceDefendRequest", response: "DisputeServiceDefendResponse" },

  // get: PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.
  get                                        : { request: "PaymentServiceGetRequest", response: "PaymentServiceGetResponse" },

  // payout_create: PayoutService.Create — Creates a payout.
  payout_create                              : { request: "PayoutServiceCreateRequest", response: "PayoutServiceCreateResponse" },

  // payout_create_link: PayoutService.CreateLink — Creates a link between the recipient and the payout.
  payout_create_link                         : { request: "PayoutServiceCreateLinkRequest", response: "PayoutServiceCreateLinkResponse" },

  // payout_create_recipient: PayoutService.CreateRecipient — Create payout recipient.
  payout_create_recipient                    : { request: "PayoutServiceCreateRecipientRequest", response: "PayoutServiceCreateRecipientResponse" },

  // payout_enroll_disburse_account: PayoutService.EnrollDisburseAccount — Enroll disburse account.
  payout_enroll_disburse_account             : { request: "PayoutServiceEnrollDisburseAccountRequest", response: "PayoutServiceEnrollDisburseAccountResponse" },

  // payout_get: PayoutService.Get — Retrieve payout details.
  payout_get                                 : { request: "PayoutServiceGetRequest", response: "PayoutServiceGetResponse" },

  // payout_stage: PayoutService.Stage — Stage the payout.
  payout_stage                               : { request: "PayoutServiceStageRequest", response: "PayoutServiceStageResponse" },

  // payout_transfer: PayoutService.Transfer — Creates a payout fund transfer.
  payout_transfer                            : { request: "PayoutServiceTransferRequest", response: "PayoutServiceTransferResponse" },

  // payout_void: PayoutService.Void — Void a payout.
  payout_void                                : { request: "PayoutServiceVoidRequest", response: "PayoutServiceVoidResponse" },

  // post_authenticate: PaymentMethodAuthenticationService.PostAuthenticate — Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed.
  post_authenticate                          : { request: "PaymentMethodAuthenticationServicePostAuthenticateRequest", response: "PaymentMethodAuthenticationServicePostAuthenticateResponse" },

  // pre_authenticate: PaymentMethodAuthenticationService.PreAuthenticate — Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.
  pre_authenticate                           : { request: "PaymentMethodAuthenticationServicePreAuthenticateRequest", response: "PaymentMethodAuthenticationServicePreAuthenticateResponse" },

  // proxy_authorize: PaymentService.ProxyAuthorize — Authorize using vault-aliased card data. Proxy substitutes before connector.
  proxy_authorize                            : { request: "PaymentServiceProxyAuthorizeRequest", response: "PaymentServiceAuthorizeResponse" },

  // proxy_setup_recurring: PaymentService.ProxySetupRecurring — Setup recurring mandate using vault-aliased card data.
  proxy_setup_recurring                      : { request: "PaymentServiceProxySetupRecurringRequest", response: "PaymentServiceSetupRecurringResponse" },

  // refund: PaymentService.Refund — Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.
  refund                                     : { request: "PaymentServiceRefundRequest", response: "RefundResponse" },

  // reverse: PaymentService.Reverse — Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.
  reverse                                    : { request: "PaymentServiceReverseRequest", response: "PaymentServiceReverseResponse" },

  // setup_recurring: PaymentService.SetupRecurring — Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.
  setup_recurring                            : { request: "PaymentServiceSetupRecurringRequest", response: "PaymentServiceSetupRecurringResponse" },

  // submit_evidence: DisputeService.SubmitEvidence — Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims.
  submit_evidence                            : { request: "DisputeServiceSubmitEvidenceRequest", response: "DisputeServiceSubmitEvidenceResponse" },

  // token_authorize: PaymentService.TokenAuthorize — Authorize using a connector-issued payment method token.
  token_authorize                            : { request: "PaymentServiceTokenAuthorizeRequest", response: "PaymentServiceAuthorizeResponse" },

  // token_setup_recurring: PaymentService.TokenSetupRecurring — Setup a recurring mandate using a connector token.
  token_setup_recurring                      : { request: "PaymentServiceTokenSetupRecurringRequest", response: "PaymentServiceSetupRecurringResponse" },

  // tokenize: PaymentMethodService.Tokenize — Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.
  tokenize                                   : { request: "PaymentMethodServiceTokenizeRequest", response: "PaymentMethodServiceTokenizeResponse" },

  // void: PaymentService.Void — Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.
  void                                       : { request: "PaymentServiceVoidRequest", response: "PaymentServiceVoidResponse" },

};

// Single-step flows: no HTTP round-trip.
const SINGLE_FLOWS = {
  // handle_event: EventService.HandleEvent — Process webhook notifications from connectors. Translates connector events into standardized responses for asynchronous payment state updates.
  handle_event : { request: "EventServiceHandleRequest", response: "EventServiceHandleResponse" },

};

module.exports = { FLOWS, SINGLE_FLOWS };
