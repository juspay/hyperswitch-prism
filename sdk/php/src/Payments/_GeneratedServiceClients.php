<?php

// AUTO-GENERATED — do not edit by hand.
// Source: services.proto ∩ bindings/uniffi.rs  |  Regenerate: make generate

declare(strict_types=1);

namespace Payments;

use Types\RequestConfig;

class CustomerClient extends ConnectorClientBase
{
    /** CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information. */
    public function create($request, ?RequestConfig $options = null): \Types\CustomerServiceCreateResponse
    {
        return $this->executeFlow('create', $request, \Types\CustomerServiceCreateResponse::class, $options);
    }

}

class DisputeClient extends ConnectorClientBase
{
    /** DisputeService.Accept — Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient. */
    public function accept($request, ?RequestConfig $options = null): \Types\DisputeServiceAcceptResponse
    {
        return $this->executeFlow('accept', $request, \Types\DisputeServiceAcceptResponse::class, $options);
    }

    /** DisputeService.Defend — Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation. */
    public function defend($request, ?RequestConfig $options = null): \Types\DisputeServiceDefendResponse
    {
        return $this->executeFlow('defend', $request, \Types\DisputeServiceDefendResponse::class, $options);
    }

    /** DisputeService.SubmitEvidence — Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims. */
    public function submitEvidence($request, ?RequestConfig $options = null): \Types\DisputeServiceSubmitEvidenceResponse
    {
        return $this->executeFlow('submit_evidence', $request, \Types\DisputeServiceSubmitEvidenceResponse::class, $options);
    }

}

class EventClient extends ConnectorClientBase
{
    /** EventService.HandleEvent — Verify webhook source and return a unified typed response. Response mirrors PaymentService.Get / RefundService.Get / DisputeService.Get. */
    public function handleEvent($request, ?RequestConfig $options = null): \Types\EventServiceHandleResponse
    {
        return $this->executeDirect('handle_event', $request, \Types\EventServiceHandleResponse::class, $options);
    }

    /** EventService.ParseEvent — Parse a raw webhook payload without credentials. Returns resource reference and event type — sufficient to resolve secrets or early-exit. */
    public function parseEvent($request, ?RequestConfig $options = null): \Types\EventServiceParseResponse
    {
        return $this->executeDirect('parse_event', $request, \Types\EventServiceParseResponse::class, $options);
    }

}

class MerchantAuthenticationClient extends ConnectorClientBase
{
    /** MerchantAuthenticationService.CreateClientAuthenticationToken — Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI. */
    public function createClientAuthenticationToken($request, ?RequestConfig $options = null): \Types\MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse
    {
        return $this->executeFlow('create_client_authentication_token', $request, \Types\MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse::class, $options);
    }

    /** MerchantAuthenticationService.CreateServerAuthenticationToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side. */
    public function createServerAuthenticationToken($request, ?RequestConfig $options = null): \Types\MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse
    {
        return $this->executeFlow('create_server_authentication_token', $request, \Types\MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse::class, $options);
    }

    /** MerchantAuthenticationService.CreateServerSessionAuthenticationToken — Create a server-side session with the connector. Establishes session state for multi-step operations like 3DS verification or wallet authorization. */
    public function createServerSessionAuthenticationToken($request, ?RequestConfig $options = null): \Types\MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse
    {
        return $this->executeFlow('create_server_session_authentication_token', $request, \Types\MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse::class, $options);
    }

}

class PaymentMethodAuthenticationClient extends ConnectorClientBase
{
    /** PaymentMethodAuthenticationService.Authenticate — Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention. */
    public function authenticate($request, ?RequestConfig $options = null): \Types\PaymentMethodAuthenticationServiceAuthenticateResponse
    {
        return $this->executeFlow('authenticate', $request, \Types\PaymentMethodAuthenticationServiceAuthenticateResponse::class, $options);
    }

    /** PaymentMethodAuthenticationService.PostAuthenticate — Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed. */
    public function postAuthenticate($request, ?RequestConfig $options = null): \Types\PaymentMethodAuthenticationServicePostAuthenticateResponse
    {
        return $this->executeFlow('post_authenticate', $request, \Types\PaymentMethodAuthenticationServicePostAuthenticateResponse::class, $options);
    }

    /** PaymentMethodAuthenticationService.PreAuthenticate — Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification. */
    public function preAuthenticate($request, ?RequestConfig $options = null): \Types\PaymentMethodAuthenticationServicePreAuthenticateResponse
    {
        return $this->executeFlow('pre_authenticate', $request, \Types\PaymentMethodAuthenticationServicePreAuthenticateResponse::class, $options);
    }

}

class PaymentMethodClient extends ConnectorClientBase
{
    /** PaymentMethodService.Tokenize — Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing. */
    public function tokenize($request, ?RequestConfig $options = null): \Types\PaymentMethodServiceTokenizeResponse
    {
        return $this->executeFlow('tokenize', $request, \Types\PaymentMethodServiceTokenizeResponse::class, $options);
    }

}

class PaymentClient extends ConnectorClientBase
{
    /** PaymentService.Authorize — Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing. */
    public function authorize($request, ?RequestConfig $options = null): \Types\PaymentServiceAuthorizeResponse
    {
        return $this->executeFlow('authorize', $request, \Types\PaymentServiceAuthorizeResponse::class, $options);
    }

    /** PaymentService.Capture — Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account. */
    public function capture($request, ?RequestConfig $options = null): \Types\PaymentServiceCaptureResponse
    {
        return $this->executeFlow('capture', $request, \Types\PaymentServiceCaptureResponse::class, $options);
    }

    /** PaymentService.CreateOrder — Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls. */
    public function createOrder($request, ?RequestConfig $options = null): \Types\PaymentServiceCreateOrderResponse
    {
        return $this->executeFlow('create_order', $request, \Types\PaymentServiceCreateOrderResponse::class, $options);
    }

    /** PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking. */
    public function get($request, ?RequestConfig $options = null): \Types\PaymentServiceGetResponse
    {
        return $this->executeFlow('get', $request, \Types\PaymentServiceGetResponse::class, $options);
    }

    /** PaymentService.IncrementalAuthorization — Increase the authorized amount for an existing payment. Enables you to capture additional funds when the transaction amount changes after initial authorization. */
    public function incrementalAuthorization($request, ?RequestConfig $options = null): \Types\PaymentServiceIncrementalAuthorizationResponse
    {
        return $this->executeFlow('incremental_authorization', $request, \Types\PaymentServiceIncrementalAuthorizationResponse::class, $options);
    }

    /** PaymentService.ProxyAuthorize — Authorize using vault-aliased card data. Proxy substitutes before connector. */
    public function proxyAuthorize($request, ?RequestConfig $options = null): \Types\PaymentServiceAuthorizeResponse
    {
        return $this->executeFlow('proxy_authorize', $request, \Types\PaymentServiceAuthorizeResponse::class, $options);
    }

    /** PaymentService.ProxySetupRecurring — Setup recurring mandate using vault-aliased card data. */
    public function proxySetupRecurring($request, ?RequestConfig $options = null): \Types\PaymentServiceSetupRecurringResponse
    {
        return $this->executeFlow('proxy_setup_recurring', $request, \Types\PaymentServiceSetupRecurringResponse::class, $options);
    }

    /** PaymentService.Refund — Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled. */
    public function refund($request, ?RequestConfig $options = null): \Types\RefundResponse
    {
        return $this->executeFlow('refund', $request, \Types\RefundResponse::class, $options);
    }

    /** PaymentService.Reverse — Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization. */
    public function reverse($request, ?RequestConfig $options = null): \Types\PaymentServiceReverseResponse
    {
        return $this->executeFlow('reverse', $request, \Types\PaymentServiceReverseResponse::class, $options);
    }

    /** PaymentService.SetupRecurring — Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges. */
    public function setupRecurring($request, ?RequestConfig $options = null): \Types\PaymentServiceSetupRecurringResponse
    {
        return $this->executeFlow('setup_recurring', $request, \Types\PaymentServiceSetupRecurringResponse::class, $options);
    }

    /** PaymentService.TokenAuthorize — Authorize using a connector-issued payment method token. */
    public function tokenAuthorize($request, ?RequestConfig $options = null): \Types\PaymentServiceAuthorizeResponse
    {
        return $this->executeFlow('token_authorize', $request, \Types\PaymentServiceAuthorizeResponse::class, $options);
    }

    /** PaymentService.TokenSetupRecurring — Setup a recurring mandate using a connector token. */
    public function tokenSetupRecurring($request, ?RequestConfig $options = null): \Types\PaymentServiceSetupRecurringResponse
    {
        return $this->executeFlow('token_setup_recurring', $request, \Types\PaymentServiceSetupRecurringResponse::class, $options);
    }

    /** PaymentService.Void — Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed. */
    public function void($request, ?RequestConfig $options = null): \Types\PaymentServiceVoidResponse
    {
        return $this->executeFlow('void', $request, \Types\PaymentServiceVoidResponse::class, $options);
    }

    /** PaymentService.VerifyRedirectResponse — Verify and process redirect responses from 3D Secure or other external flows. Validates authentication results and updates payment state accordingly. */
    public function verifyRedirectResponse($request, ?RequestConfig $options = null): \Types\PaymentServiceVerifyRedirectResponseResponse
    {
        return $this->executeDirect('verify_redirect_response', $request, \Types\PaymentServiceVerifyRedirectResponseResponse::class, $options);
    }

}

class PayoutClient extends ConnectorClientBase
{
    /** PayoutService.Create — Creates a payout. */
    public function payoutCreate($request, ?RequestConfig $options = null): \Types\PayoutServiceCreateResponse
    {
        return $this->executeFlow('payout_create', $request, \Types\PayoutServiceCreateResponse::class, $options);
    }

    /** PayoutService.CreateLink — Creates a link between the recipient and the payout. */
    public function payoutCreateLink($request, ?RequestConfig $options = null): \Types\PayoutServiceCreateLinkResponse
    {
        return $this->executeFlow('payout_create_link', $request, \Types\PayoutServiceCreateLinkResponse::class, $options);
    }

    /** PayoutService.CreateRecipient — Create payout recipient. */
    public function payoutCreateRecipient($request, ?RequestConfig $options = null): \Types\PayoutServiceCreateRecipientResponse
    {
        return $this->executeFlow('payout_create_recipient', $request, \Types\PayoutServiceCreateRecipientResponse::class, $options);
    }

    /** PayoutService.EnrollDisburseAccount — Enroll disburse account. */
    public function payoutEnrollDisburseAccount($request, ?RequestConfig $options = null): \Types\PayoutServiceEnrollDisburseAccountResponse
    {
        return $this->executeFlow('payout_enroll_disburse_account', $request, \Types\PayoutServiceEnrollDisburseAccountResponse::class, $options);
    }

    /** PayoutService.Get — Retrieve payout details. */
    public function payoutGet($request, ?RequestConfig $options = null): \Types\PayoutServiceGetResponse
    {
        return $this->executeFlow('payout_get', $request, \Types\PayoutServiceGetResponse::class, $options);
    }

    /** PayoutService.Stage — Stage the payout. */
    public function payoutStage($request, ?RequestConfig $options = null): \Types\PayoutServiceStageResponse
    {
        return $this->executeFlow('payout_stage', $request, \Types\PayoutServiceStageResponse::class, $options);
    }

    /** PayoutService.Transfer — Creates a payout fund transfer. */
    public function payoutTransfer($request, ?RequestConfig $options = null): \Types\PayoutServiceTransferResponse
    {
        return $this->executeFlow('payout_transfer', $request, \Types\PayoutServiceTransferResponse::class, $options);
    }

    /** PayoutService.Void — Void a payout. */
    public function payoutVoid($request, ?RequestConfig $options = null): \Types\PayoutServiceVoidResponse
    {
        return $this->executeFlow('payout_void', $request, \Types\PayoutServiceVoidResponse::class, $options);
    }

}

class RecurringPaymentClient extends ConnectorClientBase
{
    /** RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details. */
    public function charge($request, ?RequestConfig $options = null): \Types\RecurringPaymentServiceChargeResponse
    {
        return $this->executeFlow('charge', $request, \Types\RecurringPaymentServiceChargeResponse::class, $options);
    }

    /** RecurringPaymentService.Revoke — Cancel an existing recurring payment mandate. Stops future automatic charges on customer's stored consent for subscription cancellations. */
    public function recurringRevoke($request, ?RequestConfig $options = null): \Types\RecurringPaymentServiceRevokeResponse
    {
        return $this->executeFlow('recurring_revoke', $request, \Types\RecurringPaymentServiceRevokeResponse::class, $options);
    }

}

class RefundClient extends ConnectorClientBase
{
    /** RefundService.Get — Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication. */
    public function refundGet($request, ?RequestConfig $options = null): \Types\RefundResponse
    {
        return $this->executeFlow('refund_get', $request, \Types\RefundResponse::class, $options);
    }

}
