# AUTO-GENERATED — do not edit by hand.
# Source: services.proto ∩ bindings/uniffi.rs  |  Regenerate: make generate
#
# This stub exposes per-service client classes to static analysers
# (Pylance, pyright, mypy) so IDEs offer completions and type checking.
from payments.generated.sdk_config_pb2 import ConnectorConfig, RequestConfig
from payments.generated.payment_pb2 import (
    CustomerServiceCreateRequest,
    CustomerServiceCreateResponse,
    DisputeServiceAcceptRequest,
    DisputeServiceAcceptResponse,
    DisputeServiceDefendRequest,
    DisputeServiceDefendResponse,
    DisputeServiceSubmitEvidenceRequest,
    DisputeServiceSubmitEvidenceResponse,
    EventServiceHandleRequest,
    EventServiceHandleResponse,
    MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
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
    PaymentServiceProxyAuthorizeRequest,
    PaymentServiceProxySetupRecurringRequest,
    PaymentServiceRefundRequest,
    PaymentServiceReverseRequest,
    PaymentServiceReverseResponse,
    PaymentServiceSetupRecurringRequest,
    PaymentServiceSetupRecurringResponse,
    PaymentServiceTokenAuthorizeRequest,
    PaymentServiceTokenSetupRecurringRequest,
    PaymentServiceVoidRequest,
    PaymentServiceVoidResponse,
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
    RecurringPaymentServiceChargeRequest,
    RecurringPaymentServiceChargeResponse,
    RefundResponse,
)

class _ConnectorClientBase:
    def __init__(self, config: ConnectorConfig, defaults: RequestConfig | None = ..., lib_path: str | None = ...) -> None: ...

class CustomerClient(_ConnectorClientBase):
    def create(self, request: CustomerServiceCreateRequest, options: RequestConfig | None = ...) -> CustomerServiceCreateResponse:
        """CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information."""
        ...


class DisputeClient(_ConnectorClientBase):
    def accept(self, request: DisputeServiceAcceptRequest, options: RequestConfig | None = ...) -> DisputeServiceAcceptResponse:
        """DisputeService.Accept — Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient."""
        ...

    def defend(self, request: DisputeServiceDefendRequest, options: RequestConfig | None = ...) -> DisputeServiceDefendResponse:
        """DisputeService.Defend — Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation."""
        ...

    def submit_evidence(self, request: DisputeServiceSubmitEvidenceRequest, options: RequestConfig | None = ...) -> DisputeServiceSubmitEvidenceResponse:
        """DisputeService.SubmitEvidence — Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims."""
        ...


class EventClient(_ConnectorClientBase):
    def handle_event(self, request: EventServiceHandleRequest, options: RequestConfig | None = ...) -> EventServiceHandleResponse:
        """EventService.HandleEvent — Process webhook notifications from connectors. Translates connector events into standardized responses for asynchronous payment state updates."""
        ...


class MerchantAuthenticationClient(_ConnectorClientBase):
    def create_client_authentication_token(self, request: MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest, options: RequestConfig | None = ...) -> MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse:
        """MerchantAuthenticationService.CreateClientAuthenticationToken — Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI."""
        ...

    def create_server_authentication_token(self, request: MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest, options: RequestConfig | None = ...) -> MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse:
        """MerchantAuthenticationService.CreateServerAuthenticationToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side."""
        ...

    def create_server_session_authentication_token(self, request: MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest, options: RequestConfig | None = ...) -> MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse:
        """MerchantAuthenticationService.CreateServerSessionAuthenticationToken — Create a server-side session with the connector. Establishes session state for multi-step operations like 3DS verification or wallet authorization."""
        ...


class PaymentMethodAuthenticationClient(_ConnectorClientBase):
    def authenticate(self, request: PaymentMethodAuthenticationServiceAuthenticateRequest, options: RequestConfig | None = ...) -> PaymentMethodAuthenticationServiceAuthenticateResponse:
        """PaymentMethodAuthenticationService.Authenticate — Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention."""
        ...

    def post_authenticate(self, request: PaymentMethodAuthenticationServicePostAuthenticateRequest, options: RequestConfig | None = ...) -> PaymentMethodAuthenticationServicePostAuthenticateResponse:
        """PaymentMethodAuthenticationService.PostAuthenticate — Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed."""
        ...

    def pre_authenticate(self, request: PaymentMethodAuthenticationServicePreAuthenticateRequest, options: RequestConfig | None = ...) -> PaymentMethodAuthenticationServicePreAuthenticateResponse:
        """PaymentMethodAuthenticationService.PreAuthenticate — Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification."""
        ...


class PaymentMethodClient(_ConnectorClientBase):
    def tokenize(self, request: PaymentMethodServiceTokenizeRequest, options: RequestConfig | None = ...) -> PaymentMethodServiceTokenizeResponse:
        """PaymentMethodService.Tokenize — Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing."""
        ...


class PaymentClient(_ConnectorClientBase):
    def authorize(self, request: PaymentServiceAuthorizeRequest, options: RequestConfig | None = ...) -> PaymentServiceAuthorizeResponse:
        """PaymentService.Authorize — Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing."""
        ...

    def capture(self, request: PaymentServiceCaptureRequest, options: RequestConfig | None = ...) -> PaymentServiceCaptureResponse:
        """PaymentService.Capture — Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account."""
        ...

    def create_order(self, request: PaymentServiceCreateOrderRequest, options: RequestConfig | None = ...) -> PaymentServiceCreateOrderResponse:
        """PaymentService.CreateOrder — Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls."""
        ...

    def get(self, request: PaymentServiceGetRequest, options: RequestConfig | None = ...) -> PaymentServiceGetResponse:
        """PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking."""
        ...

    def proxy_authorize(self, request: PaymentServiceProxyAuthorizeRequest, options: RequestConfig | None = ...) -> PaymentServiceAuthorizeResponse:
        """PaymentService.ProxyAuthorize — Authorize using vault-aliased card data. Proxy substitutes before connector."""
        ...

    def proxy_setup_recurring(self, request: PaymentServiceProxySetupRecurringRequest, options: RequestConfig | None = ...) -> PaymentServiceSetupRecurringResponse:
        """PaymentService.ProxySetupRecurring — Setup recurring mandate using vault-aliased card data."""
        ...

    def refund(self, request: PaymentServiceRefundRequest, options: RequestConfig | None = ...) -> RefundResponse:
        """PaymentService.Refund — Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled."""
        ...

    def reverse(self, request: PaymentServiceReverseRequest, options: RequestConfig | None = ...) -> PaymentServiceReverseResponse:
        """PaymentService.Reverse — Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization."""
        ...

    def setup_recurring(self, request: PaymentServiceSetupRecurringRequest, options: RequestConfig | None = ...) -> PaymentServiceSetupRecurringResponse:
        """PaymentService.SetupRecurring — Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges."""
        ...

    def token_authorize(self, request: PaymentServiceTokenAuthorizeRequest, options: RequestConfig | None = ...) -> PaymentServiceAuthorizeResponse:
        """PaymentService.TokenAuthorize — Authorize using a connector-issued payment method token."""
        ...

    def token_setup_recurring(self, request: PaymentServiceTokenSetupRecurringRequest, options: RequestConfig | None = ...) -> PaymentServiceSetupRecurringResponse:
        """PaymentService.TokenSetupRecurring — Setup a recurring mandate using a connector token."""
        ...

    def void(self, request: PaymentServiceVoidRequest, options: RequestConfig | None = ...) -> PaymentServiceVoidResponse:
        """PaymentService.Void — Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed."""
        ...


class PayoutClient(_ConnectorClientBase):
    def payout_create(self, request: PayoutServiceCreateRequest, options: RequestConfig | None = ...) -> PayoutServiceCreateResponse:
        """PayoutService.Create — Creates a payout."""
        ...

    def payout_create_link(self, request: PayoutServiceCreateLinkRequest, options: RequestConfig | None = ...) -> PayoutServiceCreateLinkResponse:
        """PayoutService.CreateLink — Creates a link between the recipient and the payout."""
        ...

    def payout_create_recipient(self, request: PayoutServiceCreateRecipientRequest, options: RequestConfig | None = ...) -> PayoutServiceCreateRecipientResponse:
        """PayoutService.CreateRecipient — Create payout recipient."""
        ...

    def payout_enroll_disburse_account(self, request: PayoutServiceEnrollDisburseAccountRequest, options: RequestConfig | None = ...) -> PayoutServiceEnrollDisburseAccountResponse:
        """PayoutService.EnrollDisburseAccount — Enroll disburse account."""
        ...

    def payout_get(self, request: PayoutServiceGetRequest, options: RequestConfig | None = ...) -> PayoutServiceGetResponse:
        """PayoutService.Get — Retrieve payout details."""
        ...

    def payout_stage(self, request: PayoutServiceStageRequest, options: RequestConfig | None = ...) -> PayoutServiceStageResponse:
        """PayoutService.Stage — Stage the payout."""
        ...

    def payout_transfer(self, request: PayoutServiceTransferRequest, options: RequestConfig | None = ...) -> PayoutServiceTransferResponse:
        """PayoutService.Transfer — Creates a payout fund transfer."""
        ...

    def payout_void(self, request: PayoutServiceVoidRequest, options: RequestConfig | None = ...) -> PayoutServiceVoidResponse:
        """PayoutService.Void — Void a payout."""
        ...


class RecurringPaymentClient(_ConnectorClientBase):
    def charge(self, request: RecurringPaymentServiceChargeRequest, options: RequestConfig | None = ...) -> RecurringPaymentServiceChargeResponse:
        """RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details."""
        ...
