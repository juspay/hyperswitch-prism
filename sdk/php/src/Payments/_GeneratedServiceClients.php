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
    /** EventService.HandleEvent — Process webhook notifications from connectors. Translates connector events into standardized responses for asynchronous payment state updates. */
    public function handleEvent($request, ?RequestConfig $options = null): \Types\EventServiceHandleResponse
    {
        return $this->executeDirect('handle_event', $request, \Types\EventServiceHandleResponse::class, $options);
    }

}

class MerchantAuthenticationClient extends ConnectorClientBase
{
    /** MerchantAuthenticationService.CreateAccessToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side. */
    public function createAccessToken($request, ?RequestConfig $options = null): \Types\MerchantAuthenticationServiceCreateAccessTokenResponse
    {
        return $this->executeFlow('create_access_token', $request, \Types\MerchantAuthenticationServiceCreateAccessTokenResponse::class, $options);
    }

    /** MerchantAuthenticationService.CreateSessionToken — Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking. */
    public function createSessionToken($request, ?RequestConfig $options = null): \Types\MerchantAuthenticationServiceCreateSessionTokenResponse
    {
        return $this->executeFlow('create_session_token', $request, \Types\MerchantAuthenticationServiceCreateSessionTokenResponse::class, $options);
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

    /** PaymentService.Capture — Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle. */
    public function capture($request, ?RequestConfig $options = null): \Types\PaymentServiceCaptureResponse
    {
        return $this->executeFlow('capture', $request, \Types\PaymentServiceCaptureResponse::class, $options);
    }

    /** PaymentService.CreateOrder — Initialize an order in the payment processor system. Sets up payment context before customer enters card details for improved authorization rates. */
    public function createOrder($request, ?RequestConfig $options = null): \Types\PaymentServiceCreateOrderResponse
    {
        return $this->executeFlow('create_order', $request, \Types\PaymentServiceCreateOrderResponse::class, $options);
    }

    /** PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking. */
    public function get($request, ?RequestConfig $options = null): \Types\PaymentServiceGetResponse
    {
        return $this->executeFlow('get', $request, \Types\PaymentServiceGetResponse::class, $options);
    }

    /** PaymentService.Refund — Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment. */
    public function refund($request, ?RequestConfig $options = null): \Types\RefundResponse
    {
        return $this->executeFlow('refund', $request, \Types\RefundResponse::class, $options);
    }

    /** PaymentService.Reverse — Reverse a captured payment before settlement. Recovers funds after capture but before bank settlement, used for corrections or cancellations. */
    public function reverse($request, ?RequestConfig $options = null): \Types\PaymentServiceReverseResponse
    {
        return $this->executeFlow('reverse', $request, \Types\PaymentServiceReverseResponse::class, $options);
    }

    /** PaymentService.SetupRecurring — Setup a recurring payment instruction for future payments/ debits. This could be for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases. */
    public function setupRecurring($request, ?RequestConfig $options = null): \Types\PaymentServiceSetupRecurringResponse
    {
        return $this->executeFlow('setup_recurring', $request, \Types\PaymentServiceSetupRecurringResponse::class, $options);
    }

    /** PaymentService.Void — Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned. */
    public function void($request, ?RequestConfig $options = null): \Types\PaymentServiceVoidResponse
    {
        return $this->executeFlow('void', $request, \Types\PaymentServiceVoidResponse::class, $options);
    }

}

class RecurringPaymentClient extends ConnectorClientBase
{
    /** RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details. */
    public function charge($request, ?RequestConfig $options = null): \Types\RecurringPaymentServiceChargeResponse
    {
        return $this->executeFlow('charge', $request, \Types\RecurringPaymentServiceChargeResponse::class, $options);
    }

}
