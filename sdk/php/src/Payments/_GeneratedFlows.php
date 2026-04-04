<?php

// AUTO-GENERATED — do not edit by hand.
// Source: services.proto ∩ bindings/uniffi.rs  |  Regenerate: make generate

declare(strict_types=1);

namespace Payments;

class GeneratedFlows
{
    /**
     * Standard flows: req_transformer → HTTP → res_transformer.
     *
     * @var array<string, array{request: string, response: string}>
     */
    const FLOWS = [
        // accept: DisputeService.Accept — Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient.
        'accept'                         => ['request' => 'DisputeServiceAcceptRequest', 'response' => 'DisputeServiceAcceptResponse'],

        // authenticate: PaymentMethodAuthenticationService.Authenticate — Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.
        'authenticate'                   => ['request' => 'PaymentMethodAuthenticationServiceAuthenticateRequest', 'response' => 'PaymentMethodAuthenticationServiceAuthenticateResponse'],

        // authorize: PaymentService.Authorize — Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing.
        'authorize'                      => ['request' => 'PaymentServiceAuthorizeRequest', 'response' => 'PaymentServiceAuthorizeResponse'],

        // capture: PaymentService.Capture — Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.
        'capture'                        => ['request' => 'PaymentServiceCaptureRequest', 'response' => 'PaymentServiceCaptureResponse'],

        // charge: RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.
        'charge'                         => ['request' => 'RecurringPaymentServiceChargeRequest', 'response' => 'RecurringPaymentServiceChargeResponse'],

        // create: CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.
        'create'                         => ['request' => 'CustomerServiceCreateRequest', 'response' => 'CustomerServiceCreateResponse'],

        // create_access_token: MerchantAuthenticationService.CreateAccessToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.
        'create_access_token'            => ['request' => 'MerchantAuthenticationServiceCreateAccessTokenRequest', 'response' => 'MerchantAuthenticationServiceCreateAccessTokenResponse'],

        // create_order: PaymentService.CreateOrder — Initialize an order in the payment processor system. Sets up payment context before customer enters card details for improved authorization rates.
        'create_order'                   => ['request' => 'PaymentServiceCreateOrderRequest', 'response' => 'PaymentServiceCreateOrderResponse'],

        // create_session_token: MerchantAuthenticationService.CreateSessionToken — Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking.
        'create_session_token'           => ['request' => 'MerchantAuthenticationServiceCreateSessionTokenRequest', 'response' => 'MerchantAuthenticationServiceCreateSessionTokenResponse'],

        // defend: DisputeService.Defend — Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation.
        'defend'                         => ['request' => 'DisputeServiceDefendRequest', 'response' => 'DisputeServiceDefendResponse'],

        // get: PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.
        'get'                            => ['request' => 'PaymentServiceGetRequest', 'response' => 'PaymentServiceGetResponse'],

        // payout_create: PayoutService.Create — Creates a payout.
        'payout_create'                  => ['request' => 'PayoutServiceCreateRequest', 'response' => 'PayoutServiceCreateResponse'],

        // payout_create_link: PayoutService.CreateLink — Creates a link between the recipient and the payout.
        'payout_create_link'             => ['request' => 'PayoutServiceCreateLinkRequest', 'response' => 'PayoutServiceCreateLinkResponse'],

        // payout_create_recipient: PayoutService.CreateRecipient — Create payout recipient.
        'payout_create_recipient'        => ['request' => 'PayoutServiceCreateRecipientRequest', 'response' => 'PayoutServiceCreateRecipientResponse'],

        // payout_enroll_disburse_account: PayoutService.EnrollDisburseAccount — Enroll disburse account.
        'payout_enroll_disburse_account' => ['request' => 'PayoutServiceEnrollDisburseAccountRequest', 'response' => 'PayoutServiceEnrollDisburseAccountResponse'],

        // payout_get: PayoutService.Get — Retrieve payout details.
        'payout_get'                     => ['request' => 'PayoutServiceGetRequest', 'response' => 'PayoutServiceGetResponse'],

        // payout_stage: PayoutService.Stage — Stage the payout.
        'payout_stage'                   => ['request' => 'PayoutServiceStageRequest', 'response' => 'PayoutServiceStageResponse'],

        // payout_transfer: PayoutService.Transfer — Creates a payout fund transfer.
        'payout_transfer'                => ['request' => 'PayoutServiceTransferRequest', 'response' => 'PayoutServiceTransferResponse'],

        // payout_void: PayoutService.Void — Void a payout.
        'payout_void'                    => ['request' => 'PayoutServiceVoidRequest', 'response' => 'PayoutServiceVoidResponse'],

        // post_authenticate: PaymentMethodAuthenticationService.PostAuthenticate — Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed.
        'post_authenticate'              => ['request' => 'PaymentMethodAuthenticationServicePostAuthenticateRequest', 'response' => 'PaymentMethodAuthenticationServicePostAuthenticateResponse'],

        // pre_authenticate: PaymentMethodAuthenticationService.PreAuthenticate — Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.
        'pre_authenticate'               => ['request' => 'PaymentMethodAuthenticationServicePreAuthenticateRequest', 'response' => 'PaymentMethodAuthenticationServicePreAuthenticateResponse'],

        // refund: PaymentService.Refund — Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.
        'refund'                         => ['request' => 'PaymentServiceRefundRequest', 'response' => 'RefundResponse'],

        // reverse: PaymentService.Reverse — Reverse a captured payment before settlement. Recovers funds after capture but before bank settlement, used for corrections or cancellations.
        'reverse'                        => ['request' => 'PaymentServiceReverseRequest', 'response' => 'PaymentServiceReverseResponse'],

        // setup_recurring: PaymentService.SetupRecurring — Setup a recurring payment instruction for future payments/ debits. This could be for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.
        'setup_recurring'                => ['request' => 'PaymentServiceSetupRecurringRequest', 'response' => 'PaymentServiceSetupRecurringResponse'],

        // submit_evidence: DisputeService.SubmitEvidence — Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims.
        'submit_evidence'                => ['request' => 'DisputeServiceSubmitEvidenceRequest', 'response' => 'DisputeServiceSubmitEvidenceResponse'],

        // tokenize: PaymentMethodService.Tokenize — Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.
        'tokenize'                       => ['request' => 'PaymentMethodServiceTokenizeRequest', 'response' => 'PaymentMethodServiceTokenizeResponse'],

        // void: PaymentService.Void — Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.
        'void'                           => ['request' => 'PaymentServiceVoidRequest', 'response' => 'PaymentServiceVoidResponse'],

    ];

    /**
     * Single-step flows: transformer called directly, no HTTP round-trip.
     * Used for inbound flows such as webhook processing.
     *
     * @var array<string, array{request: string, response: string}>
     */
    const SINGLE_FLOWS = [
        // handle_event: EventService.HandleEvent — Process webhook notifications from connectors. Translates connector events into standardized responses for asynchronous payment state updates.
        'handle_event' => ['request' => 'EventServiceHandleRequest', 'response' => 'EventServiceHandleResponse'],

    ];
}
