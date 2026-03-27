// AUTO-GENERATED — do not edit by hand.
// Source: services.proto ∩ bindings/uniffi.rs  |  Regenerate: make generate

import { ConnectorClient as _ConnectorClientBase } from "./connector_client";
// @ts-ignore - protobuf generated files might not have types yet
import { types } from "./generated/proto";

export class CustomerClient extends _ConnectorClientBase {
  /** CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information. */
  async create(
    requestMsg: types.ICustomerServiceCreateRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.CustomerServiceCreateResponse> {
    return this._executeFlow('create', requestMsg, options, 'CustomerServiceCreateRequest', 'CustomerServiceCreateResponse') as Promise<types.CustomerServiceCreateResponse>;
  }

}

export class DirectPaymentClient extends _ConnectorClientBase {
  /** DirectPaymentService.Authorize — Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing. */
  async authorize(
    requestMsg: types.IPaymentServiceAuthorizeRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentServiceAuthorizeResponse> {
    return this._executeFlow('authorize', requestMsg, options, 'PaymentServiceAuthorizeRequest', 'PaymentServiceAuthorizeResponse') as Promise<types.PaymentServiceAuthorizeResponse>;
  }

  /** DirectPaymentService.Capture — Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account. */
  async capture(
    requestMsg: types.IPaymentServiceCaptureRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentServiceCaptureResponse> {
    return this._executeFlow('capture', requestMsg, options, 'PaymentServiceCaptureRequest', 'PaymentServiceCaptureResponse') as Promise<types.PaymentServiceCaptureResponse>;
  }

  /** DirectPaymentService.CreateOrder — Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls. */
  async createOrder(
    requestMsg: types.IPaymentServiceCreateOrderRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentServiceCreateOrderResponse> {
    return this._executeFlow('create_order', requestMsg, options, 'PaymentServiceCreateOrderRequest', 'PaymentServiceCreateOrderResponse') as Promise<types.PaymentServiceCreateOrderResponse>;
  }

  /** DirectPaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking. */
  async get(
    requestMsg: types.IPaymentServiceGetRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentServiceGetResponse> {
    return this._executeFlow('get', requestMsg, options, 'PaymentServiceGetRequest', 'PaymentServiceGetResponse') as Promise<types.PaymentServiceGetResponse>;
  }

  /** DirectPaymentService.Refund — Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled. */
  async refund(
    requestMsg: types.IPaymentServiceRefundRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.RefundResponse> {
    return this._executeFlow('refund', requestMsg, options, 'PaymentServiceRefundRequest', 'RefundResponse') as Promise<types.RefundResponse>;
  }

  /** DirectPaymentService.Reverse — Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization. */
  async reverse(
    requestMsg: types.IPaymentServiceReverseRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentServiceReverseResponse> {
    return this._executeFlow('reverse', requestMsg, options, 'PaymentServiceReverseRequest', 'PaymentServiceReverseResponse') as Promise<types.PaymentServiceReverseResponse>;
  }

  /** DirectPaymentService.SetupRecurring — Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges. */
  async setupRecurring(
    requestMsg: types.IPaymentServiceSetupRecurringRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentServiceSetupRecurringResponse> {
    return this._executeFlow('setup_recurring', requestMsg, options, 'PaymentServiceSetupRecurringRequest', 'PaymentServiceSetupRecurringResponse') as Promise<types.PaymentServiceSetupRecurringResponse>;
  }

  /** DirectPaymentService.Void — Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed. */
  async void(
    requestMsg: types.IPaymentServiceVoidRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentServiceVoidResponse> {
    return this._executeFlow('void', requestMsg, options, 'PaymentServiceVoidRequest', 'PaymentServiceVoidResponse') as Promise<types.PaymentServiceVoidResponse>;
  }

}

export class DisputeClient extends _ConnectorClientBase {
  /** DisputeService.Accept — Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient. */
  async accept(
    requestMsg: types.IDisputeServiceAcceptRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.DisputeServiceAcceptResponse> {
    return this._executeFlow('accept', requestMsg, options, 'DisputeServiceAcceptRequest', 'DisputeServiceAcceptResponse') as Promise<types.DisputeServiceAcceptResponse>;
  }

  /** DisputeService.Defend — Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation. */
  async defend(
    requestMsg: types.IDisputeServiceDefendRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.DisputeServiceDefendResponse> {
    return this._executeFlow('defend', requestMsg, options, 'DisputeServiceDefendRequest', 'DisputeServiceDefendResponse') as Promise<types.DisputeServiceDefendResponse>;
  }

  /** DisputeService.SubmitEvidence — Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims. */
  async submitEvidence(
    requestMsg: types.IDisputeServiceSubmitEvidenceRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.DisputeServiceSubmitEvidenceResponse> {
    return this._executeFlow('submit_evidence', requestMsg, options, 'DisputeServiceSubmitEvidenceRequest', 'DisputeServiceSubmitEvidenceResponse') as Promise<types.DisputeServiceSubmitEvidenceResponse>;
  }

}

export class EventClient extends _ConnectorClientBase {
  /** EventService.HandleEvent — Process webhook notifications from connectors. Translates connector events into standardized responses for asynchronous payment state updates. */
  async handleEvent(
    requestMsg: types.IEventServiceHandleRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.EventServiceHandleResponse> {
    return this._executeDirect('handle_event', requestMsg, options, 'EventServiceHandleRequest', 'EventServiceHandleResponse') as Promise<types.EventServiceHandleResponse>;
  }

}

export class MerchantAuthenticationClient extends _ConnectorClientBase {
  /** MerchantAuthenticationService.CreateAccessToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side. */
  async createAccessToken(
    requestMsg: types.IMerchantAuthenticationServiceCreateAccessTokenRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.MerchantAuthenticationServiceCreateAccessTokenResponse> {
    return this._executeFlow('create_access_token', requestMsg, options, 'MerchantAuthenticationServiceCreateAccessTokenRequest', 'MerchantAuthenticationServiceCreateAccessTokenResponse') as Promise<types.MerchantAuthenticationServiceCreateAccessTokenResponse>;
  }

  /** MerchantAuthenticationService.CreateSessionToken — Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking. */
  async createSessionToken(
    requestMsg: types.IMerchantAuthenticationServiceCreateSessionTokenRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.MerchantAuthenticationServiceCreateSessionTokenResponse> {
    return this._executeFlow('create_session_token', requestMsg, options, 'MerchantAuthenticationServiceCreateSessionTokenRequest', 'MerchantAuthenticationServiceCreateSessionTokenResponse') as Promise<types.MerchantAuthenticationServiceCreateSessionTokenResponse>;
  }

}

export class PaymentMethodAuthenticationClient extends _ConnectorClientBase {
  /** PaymentMethodAuthenticationService.Authenticate — Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention. */
  async authenticate(
    requestMsg: types.IPaymentMethodAuthenticationServiceAuthenticateRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentMethodAuthenticationServiceAuthenticateResponse> {
    return this._executeFlow('authenticate', requestMsg, options, 'PaymentMethodAuthenticationServiceAuthenticateRequest', 'PaymentMethodAuthenticationServiceAuthenticateResponse') as Promise<types.PaymentMethodAuthenticationServiceAuthenticateResponse>;
  }

  /** PaymentMethodAuthenticationService.PostAuthenticate — Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed. */
  async postAuthenticate(
    requestMsg: types.IPaymentMethodAuthenticationServicePostAuthenticateRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentMethodAuthenticationServicePostAuthenticateResponse> {
    return this._executeFlow('post_authenticate', requestMsg, options, 'PaymentMethodAuthenticationServicePostAuthenticateRequest', 'PaymentMethodAuthenticationServicePostAuthenticateResponse') as Promise<types.PaymentMethodAuthenticationServicePostAuthenticateResponse>;
  }

  /** PaymentMethodAuthenticationService.PreAuthenticate — Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification. */
  async preAuthenticate(
    requestMsg: types.IPaymentMethodAuthenticationServicePreAuthenticateRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentMethodAuthenticationServicePreAuthenticateResponse> {
    return this._executeFlow('pre_authenticate', requestMsg, options, 'PaymentMethodAuthenticationServicePreAuthenticateRequest', 'PaymentMethodAuthenticationServicePreAuthenticateResponse') as Promise<types.PaymentMethodAuthenticationServicePreAuthenticateResponse>;
  }

}

export class PaymentMethodClient extends _ConnectorClientBase {
  /** PaymentMethodService.Tokenize — Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing. */
  async tokenize(
    requestMsg: types.IPaymentMethodServiceTokenizeRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentMethodServiceTokenizeResponse> {
    return this._executeFlow('tokenize', requestMsg, options, 'PaymentMethodServiceTokenizeRequest', 'PaymentMethodServiceTokenizeResponse') as Promise<types.PaymentMethodServiceTokenizeResponse>;
  }

}

export class PayoutClient extends _ConnectorClientBase {
  /** PayoutService.Create — Creates a payout. */
  async payoutCreate(
    requestMsg: types.IPayoutServiceCreateRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PayoutServiceCreateResponse> {
    return this._executeFlow('payout_create', requestMsg, options, 'PayoutServiceCreateRequest', 'PayoutServiceCreateResponse') as Promise<types.PayoutServiceCreateResponse>;
  }

  /** PayoutService.CreateLink — Creates a link between the recipient and the payout. */
  async payoutCreateLink(
    requestMsg: types.IPayoutServiceCreateLinkRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PayoutServiceCreateLinkResponse> {
    return this._executeFlow('payout_create_link', requestMsg, options, 'PayoutServiceCreateLinkRequest', 'PayoutServiceCreateLinkResponse') as Promise<types.PayoutServiceCreateLinkResponse>;
  }

  /** PayoutService.CreateRecipient — Create payout recipient. */
  async payoutCreateRecipient(
    requestMsg: types.IPayoutServiceCreateRecipientRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PayoutServiceCreateRecipientResponse> {
    return this._executeFlow('payout_create_recipient', requestMsg, options, 'PayoutServiceCreateRecipientRequest', 'PayoutServiceCreateRecipientResponse') as Promise<types.PayoutServiceCreateRecipientResponse>;
  }

  /** PayoutService.EnrollDisburseAccount — Enroll disburse account. */
  async payoutEnrollDisburseAccount(
    requestMsg: types.IPayoutServiceEnrollDisburseAccountRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PayoutServiceEnrollDisburseAccountResponse> {
    return this._executeFlow('payout_enroll_disburse_account', requestMsg, options, 'PayoutServiceEnrollDisburseAccountRequest', 'PayoutServiceEnrollDisburseAccountResponse') as Promise<types.PayoutServiceEnrollDisburseAccountResponse>;
  }

  /** PayoutService.Get — Retrieve payout details. */
  async payoutGet(
    requestMsg: types.IPayoutServiceGetRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PayoutServiceGetResponse> {
    return this._executeFlow('payout_get', requestMsg, options, 'PayoutServiceGetRequest', 'PayoutServiceGetResponse') as Promise<types.PayoutServiceGetResponse>;
  }

  /** PayoutService.Stage — Stage the payout. */
  async payoutStage(
    requestMsg: types.IPayoutServiceStageRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PayoutServiceStageResponse> {
    return this._executeFlow('payout_stage', requestMsg, options, 'PayoutServiceStageRequest', 'PayoutServiceStageResponse') as Promise<types.PayoutServiceStageResponse>;
  }

  /** PayoutService.Transfer — Creates a payout fund transfer. */
  async payoutTransfer(
    requestMsg: types.IPayoutServiceTransferRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PayoutServiceTransferResponse> {
    return this._executeFlow('payout_transfer', requestMsg, options, 'PayoutServiceTransferRequest', 'PayoutServiceTransferResponse') as Promise<types.PayoutServiceTransferResponse>;
  }

  /** PayoutService.Void — Void a payout. */
  async payoutVoid(
    requestMsg: types.IPayoutServiceVoidRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PayoutServiceVoidResponse> {
    return this._executeFlow('payout_void', requestMsg, options, 'PayoutServiceVoidRequest', 'PayoutServiceVoidResponse') as Promise<types.PayoutServiceVoidResponse>;
  }

}

export class ProxiedPaymentClient extends _ConnectorClientBase {
  /** ProxiedPaymentService.Authorize — Authorize using vault-aliased card data. Proxy substitutes before connector. */
  async proxiedAuthorize(
    requestMsg: types.IProxiedPaymentServiceAuthorizeRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentServiceAuthorizeResponse> {
    return this._executeFlow('proxied_authorize', requestMsg, options, 'ProxiedPaymentServiceAuthorizeRequest', 'PaymentServiceAuthorizeResponse') as Promise<types.PaymentServiceAuthorizeResponse>;
  }

  /** ProxiedPaymentService.SetupRecurring — Setup recurring mandate using vault-aliased card data. */
  async proxiedSetupRecurring(
    requestMsg: types.IProxiedPaymentServiceSetupRecurringRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentServiceSetupRecurringResponse> {
    return this._executeFlow('proxied_setup_recurring', requestMsg, options, 'ProxiedPaymentServiceSetupRecurringRequest', 'PaymentServiceSetupRecurringResponse') as Promise<types.PaymentServiceSetupRecurringResponse>;
  }

}

export class RecurringPaymentClient extends _ConnectorClientBase {
  /** RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details. */
  async charge(
    requestMsg: types.IRecurringPaymentServiceChargeRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.RecurringPaymentServiceChargeResponse> {
    return this._executeFlow('charge', requestMsg, options, 'RecurringPaymentServiceChargeRequest', 'RecurringPaymentServiceChargeResponse') as Promise<types.RecurringPaymentServiceChargeResponse>;
  }

}

export class TokenizedPaymentClient extends _ConnectorClientBase {
  /** TokenizedPaymentService.Authorize — Authorize using a connector-issued payment method token. */
  async tokenizedAuthorize(
    requestMsg: types.ITokenizedPaymentServiceAuthorizeRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentServiceAuthorizeResponse> {
    return this._executeFlow('tokenized_authorize', requestMsg, options, 'TokenizedPaymentServiceAuthorizeRequest', 'PaymentServiceAuthorizeResponse') as Promise<types.PaymentServiceAuthorizeResponse>;
  }

  /** TokenizedPaymentService.SetupRecurring — Setup a recurring mandate using a connector token. */
  async tokenizedSetupRecurring(
    requestMsg: types.ITokenizedPaymentServiceSetupRecurringRequest,
    options?: types.IRequestConfig | null
  ): Promise<types.PaymentServiceSetupRecurringResponse> {
    return this._executeFlow('tokenized_setup_recurring', requestMsg, options, 'TokenizedPaymentServiceSetupRecurringRequest', 'PaymentServiceSetupRecurringResponse') as Promise<types.PaymentServiceSetupRecurringResponse>;
  }

}
