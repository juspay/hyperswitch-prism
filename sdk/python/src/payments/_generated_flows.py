# AUTO-GENERATED — do not edit by hand.
# Source: services.proto ∩ bindings/uniffi.rs  |  Regenerate: make generate
SERVICE_FLOWS = {
    "DisputeClient": {
        # accept: DisputeService.Accept — Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient.
        "accept": "DisputeServiceAcceptResponse",
        # defend: DisputeService.Defend — Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation.
        "defend": "DisputeServiceDefendResponse",
        # submit_evidence: DisputeService.SubmitEvidence — Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims.
        "submit_evidence": "DisputeServiceSubmitEvidenceResponse",
    },
    "PaymentMethodAuthenticationClient": {
        # authenticate: PaymentMethodAuthenticationService.Authenticate — Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.
        "authenticate": "PaymentMethodAuthenticationServiceAuthenticateResponse",
        # post_authenticate: PaymentMethodAuthenticationService.PostAuthenticate — Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed.
        "post_authenticate": "PaymentMethodAuthenticationServicePostAuthenticateResponse",
        # pre_authenticate: PaymentMethodAuthenticationService.PreAuthenticate — Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.
        "pre_authenticate": "PaymentMethodAuthenticationServicePreAuthenticateResponse",
    },
    "PaymentClient": {
        # authorize: PaymentService.Authorize — Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing.
        "authorize": "PaymentServiceAuthorizeResponse",
        # capture: PaymentService.Capture — Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.
        "capture": "PaymentServiceCaptureResponse",
        # create_order: PaymentService.CreateOrder — Initialize an order in the payment processor system. Sets up payment context before customer enters card details for improved authorization rates.
        "create_order": "PaymentServiceCreateOrderResponse",
        # get: PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.
        "get": "PaymentServiceGetResponse",
        # refund: PaymentService.Refund — Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.
        "refund": "RefundResponse",
        # reverse: PaymentService.Reverse — Reverse a captured payment before settlement. Recovers funds after capture but before bank settlement, used for corrections or cancellations.
        "reverse": "PaymentServiceReverseResponse",
        # setup_recurring: PaymentService.SetupRecurring — Setup a recurring payment instruction for future payments/ debits. This could be for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.
        "setup_recurring": "PaymentServiceSetupRecurringResponse",
        # void: PaymentService.Void — Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.
        "void": "PaymentServiceVoidResponse",
    },
    "RecurringPaymentClient": {
        # charge: RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.
        "charge": "RecurringPaymentServiceChargeResponse",
    },
    "CustomerClient": {
        # create: CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.
        "create": "CustomerServiceCreateResponse",
    },
    "MerchantAuthenticationClient": {
        # create_access_token: MerchantAuthenticationService.CreateAccessToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.
        "create_access_token": "MerchantAuthenticationServiceCreateAccessTokenResponse",
        # create_session_token: MerchantAuthenticationService.CreateSessionToken — Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking.
        "create_session_token": "MerchantAuthenticationServiceCreateSessionTokenResponse",
    },
    "PayoutClient": {
        # payout_create: PayoutService.Create — Creates a payout.
        "payout_create": "PayoutServiceCreateResponse",
        # payout_create_link: PayoutService.CreateLink — Creates a link between the recipient and the payout.
        "payout_create_link": "PayoutServiceCreateLinkResponse",
        # payout_create_recipient: PayoutService.CreateRecipient — Create payout recipient.
        "payout_create_recipient": "PayoutServiceCreateRecipientResponse",
        # payout_enroll_disburse_account: PayoutService.EnrollDisburseAccount — Enroll disburse account.
        "payout_enroll_disburse_account": "PayoutServiceEnrollDisburseAccountResponse",
        # payout_get: PayoutService.Get — Retrieve payout details.
        "payout_get": "PayoutServiceGetResponse",
        # payout_stage: PayoutService.Stage — Stage the payout.
        "payout_stage": "PayoutServiceStageResponse",
        # payout_transfer: PayoutService.Transfer — Creates a payout fund transfer.
        "payout_transfer": "PayoutServiceTransferResponse",
        # payout_void: PayoutService.Void — Void a payout.
        "payout_void": "PayoutServiceVoidResponse",
    },
    "PaymentMethodClient": {
        # tokenize: PaymentMethodService.Tokenize — Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.
        "tokenize": "PaymentMethodServiceTokenizeResponse",
    },
}

# Single-step flows: no HTTP round-trip (e.g. webhook processing).
SINGLE_SERVICE_FLOWS = {
    "EventClient": {
        # handle_event: EventService.HandleEvent — Process webhook notifications from connectors. Translates connector events into standardized responses for asynchronous payment state updates.
        "handle_event": "EventServiceHandleResponse",
    },
}
