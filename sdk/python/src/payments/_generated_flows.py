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
        # capture: PaymentService.Capture — Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.
        "capture": "PaymentServiceCaptureResponse",
        # create_order: PaymentService.CreateOrder — Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.
        "create_order": "PaymentServiceCreateOrderResponse",
        # get: PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.
        "get": "PaymentServiceGetResponse",
        # incremental_authorization: PaymentService.IncrementalAuthorization — Increase the authorized amount for an existing payment. Enables you to capture additional funds when the transaction amount changes after initial authorization.
        "incremental_authorization": "PaymentServiceIncrementalAuthorizationResponse",
        # proxy_authorize: PaymentService.ProxyAuthorize — Authorize using vault-aliased card data. Proxy substitutes before connector.
        "proxy_authorize": "PaymentServiceAuthorizeResponse",
        # proxy_setup_recurring: PaymentService.ProxySetupRecurring — Setup recurring mandate using vault-aliased card data.
        "proxy_setup_recurring": "PaymentServiceSetupRecurringResponse",
        # refund: PaymentService.Refund — Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.
        "refund": "RefundResponse",
        # reverse: PaymentService.Reverse — Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.
        "reverse": "PaymentServiceReverseResponse",
        # setup_recurring: PaymentService.SetupRecurring — Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.
        "setup_recurring": "PaymentServiceSetupRecurringResponse",
        # token_authorize: PaymentService.TokenAuthorize — Authorize using a connector-issued payment method token.
        "token_authorize": "PaymentServiceAuthorizeResponse",
        # token_setup_recurring: PaymentService.TokenSetupRecurring — Setup a recurring mandate using a connector token.
        "token_setup_recurring": "PaymentServiceSetupRecurringResponse",
        # void: PaymentService.Void — Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.
        "void": "PaymentServiceVoidResponse",
    },
    "RecurringPaymentClient": {
        # charge: RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.
        "charge": "RecurringPaymentServiceChargeResponse",
        # recurring_revoke: RecurringPaymentService.Revoke — Cancel an existing recurring payment mandate. Stops future automatic charges on customer's stored consent for subscription cancellations.
        "recurring_revoke": "RecurringPaymentServiceRevokeResponse",
    },
    "CustomerClient": {
        # create: CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.
        "create": "CustomerServiceCreateResponse",
    },
    "MerchantAuthenticationClient": {
        # create_client_authentication_token: MerchantAuthenticationService.CreateClientAuthenticationToken — Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.
        "create_client_authentication_token": "MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse",
        # create_server_authentication_token: MerchantAuthenticationService.CreateServerAuthenticationToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.
        "create_server_authentication_token": "MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse",
        # create_server_session_authentication_token: MerchantAuthenticationService.CreateServerSessionAuthenticationToken — Create a server-side session with the connector. Establishes session state for multi-step operations like 3DS verification or wallet authorization.
        "create_server_session_authentication_token": "MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse",
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
    "RefundClient": {
        # refund_get: RefundService.Get — Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.
        "refund_get": "RefundResponse",
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
    "PaymentClient": {
        # verify_redirect_response: PaymentService.VerifyRedirectResponse — Verify and process redirect responses from 3D Secure or other external flows. Validates authentication results and updates payment state accordingly.
        "verify_redirect_response": "PaymentServiceVerifyRedirectResponseResponse",
    },
}
