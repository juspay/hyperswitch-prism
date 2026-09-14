# AUTO-GENERATED — do not edit by hand.
# Source: services.proto ∩ bindings/uniffi.rs  |  Regenerate: make generate

from payments.connector_client import _ConnectorClientBase
from payments.generated import events_pb2, frm_pb2, payment_pb2, payouts_pb2, surcharge_pb2

class CustomerClient(_ConnectorClientBase):
    """CustomerService flows"""

    def customer_create(self, request, options=None):
        """CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information."""
        return self._execute_flow("customer_create", request, payment_pb2.CustomerServiceCreateResponse, options)

    def customer_get(self, request, options=None):
        """CustomerService.Get — Retrieves customer details from the payment processor. Callers typically use this before Create to implement get-or-create semantics for connectors that reject duplicates (e.g. Glomopay)."""
        return self._execute_flow("customer_get", request, payment_pb2.CustomerServiceGetResponse, options)

class DisputeClient(_ConnectorClientBase):
    """DisputeService flows"""

    def accept(self, request, options=None):
        """DisputeService.Accept — Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient."""
        return self._execute_flow("accept", request, payment_pb2.DisputeServiceAcceptResponse, options)

    def defend(self, request, options=None):
        """DisputeService.Defend — Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation."""
        return self._execute_flow("defend", request, payment_pb2.DisputeServiceDefendResponse, options)

    def submit_evidence(self, request, options=None):
        """DisputeService.SubmitEvidence — Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims."""
        return self._execute_flow("submit_evidence", request, payment_pb2.DisputeServiceSubmitEvidenceResponse, options)

class EventClient(_ConnectorClientBase):
    """EventService flows"""

    def handle_event(self, request, options=None):
        """EventService.HandleEvent — Verify webhook source and return a unified typed response. Response mirrors PaymentService.Get / RefundService.Get / DisputeService.Get."""
        return self._execute_direct("handle_event", request, events_pb2.EventServiceHandleResponse, options)

    def parse_event(self, request, options=None):
        """EventService.ParseEvent — Parse a raw webhook payload without credentials. Returns resource reference and event type — sufficient to resolve secrets or early-exit."""
        return self._execute_direct("parse_event", request, events_pb2.EventServiceParseResponse, options)

class FraudAndRiskManagementClient(_ConnectorClientBase):
    """FraudAndRiskManagementService flows"""

    def post_risk_check(self, request, options=None):
        """FraudAndRiskManagementService.PostRiskCheck — Evaluate fraud risk after payment processing. Analyzes payment outcomes and post-transaction signals to refine risk models and detect chargeback fraud."""
        return self._execute_flow("post_risk_check", request, frm_pb2.FrmServicePostRiskCheckResponse, options)

    def pre_risk_check(self, request, options=None):
        """FraudAndRiskManagementService.PreRiskCheck — Evaluate fraud risk before payment processing. Analyzes transaction details, customer behavior, and device fingerprints to determine if the payment should proceed, be rejected, or flagged for manual review."""
        return self._execute_flow("pre_risk_check", request, frm_pb2.FrmServicePreRiskCheckResponse, options)

class MerchantAuthenticationClient(_ConnectorClientBase):
    """MerchantAuthenticationService flows"""

    def create_client_authentication_token(self, request, options=None):
        """MerchantAuthenticationService.CreateClientAuthenticationToken — Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI."""
        return self._execute_flow("create_client_authentication_token", request, payment_pb2.MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse, options)

    def create_server_authentication_token(self, request, options=None):
        """MerchantAuthenticationService.CreateServerAuthenticationToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side."""
        return self._execute_flow("create_server_authentication_token", request, payment_pb2.MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse, options)

    def create_server_session_authentication_token(self, request, options=None):
        """MerchantAuthenticationService.CreateServerSessionAuthenticationToken — Create a server-side session with the connector. Establishes session state for multi-step operations like 3DS verification or wallet authorization."""
        return self._execute_flow("create_server_session_authentication_token", request, payment_pb2.MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse, options)

class PaymentMethodAuthenticationClient(_ConnectorClientBase):
    """PaymentMethodAuthenticationService flows"""

    def authenticate(self, request, options=None):
        """PaymentMethodAuthenticationService.Authenticate — Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention."""
        return self._execute_flow("authenticate", request, payment_pb2.PaymentMethodAuthenticationServiceAuthenticateResponse, options)

    def post_authenticate(self, request, options=None):
        """PaymentMethodAuthenticationService.PostAuthenticate — Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed."""
        return self._execute_flow("post_authenticate", request, payment_pb2.PaymentMethodAuthenticationServicePostAuthenticateResponse, options)

    def pre_authenticate(self, request, options=None):
        """PaymentMethodAuthenticationService.PreAuthenticate — Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification."""
        return self._execute_flow("pre_authenticate", request, payment_pb2.PaymentMethodAuthenticationServicePreAuthenticateResponse, options)

class PaymentMethodClient(_ConnectorClientBase):
    """PaymentMethodService flows"""

    def eligibility(self, request, options=None):
        """PaymentMethodService.Eligibility — Check if the payment method is eligible for the transaction (e.g. BNPL pre-checkout check)"""
        return self._execute_flow("eligibility", request, payment_pb2.PaymentMethodServiceEligibilityResponse, options)

    def refresh(self, request, options=None):
        """PaymentMethodService.Refresh — Refresh a payment method the caller already holds in full. The request carries the instrument itself, not a reference to it: use Refresh when you own the complete payment method details and the provider exposes an endpoint that evaluates them."""
        return self._execute_flow("refresh", request, payment_pb2.PaymentMethodServiceRefreshResponse, options)

    def tokenize(self, request, options=None):
        """PaymentMethodService.Tokenize — Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing."""
        return self._execute_flow("tokenize", request, payment_pb2.PaymentMethodServiceTokenizeResponse, options)

class PaymentClient(_ConnectorClientBase):
    """PaymentService flows"""

    def authorize(self, request, options=None):
        """PaymentService.Authorize — Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing."""
        return self._execute_flow("authorize", request, payment_pb2.PaymentServiceAuthorizeResponse, options)

    def capture(self, request, options=None):
        """PaymentService.Capture — Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account."""
        return self._execute_flow("capture", request, payment_pb2.PaymentServiceCaptureResponse, options)

    def create_order(self, request, options=None):
        """PaymentService.CreateOrder — Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls."""
        return self._execute_flow("create_order", request, payment_pb2.PaymentServiceCreateOrderResponse, options)

    def get(self, request, options=None):
        """PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking."""
        return self._execute_flow("get", request, payment_pb2.PaymentServiceGetResponse, options)

    def incremental_authorization(self, request, options=None):
        """PaymentService.IncrementalAuthorization — Increase the authorized amount for an existing payment. Enables you to capture additional funds when the transaction amount changes after initial authorization."""
        return self._execute_flow("incremental_authorization", request, payment_pb2.PaymentServiceIncrementalAuthorizationResponse, options)

    def proxy_authorize(self, request, options=None):
        """PaymentService.ProxyAuthorize — Authorize using vault-aliased card data. Proxy substitutes before connector."""
        return self._execute_flow("proxy_authorize", request, payment_pb2.PaymentServiceAuthorizeResponse, options)

    def proxy_setup_recurring(self, request, options=None):
        """PaymentService.ProxySetupRecurring — Setup recurring mandate using vault-aliased card data."""
        return self._execute_flow("proxy_setup_recurring", request, payment_pb2.PaymentServiceSetupRecurringResponse, options)

    def refund(self, request, options=None):
        """PaymentService.Refund — Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled."""
        return self._execute_flow("refund", request, payment_pb2.RefundResponse, options)

    def reverse(self, request, options=None):
        """PaymentService.Reverse — Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization."""
        return self._execute_flow("reverse", request, payment_pb2.PaymentServiceReverseResponse, options)

    def setup_recurring(self, request, options=None):
        """PaymentService.SetupRecurring — Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges."""
        return self._execute_flow("setup_recurring", request, payment_pb2.PaymentServiceSetupRecurringResponse, options)

    def token_authorize(self, request, options=None):
        """PaymentService.TokenAuthorize — Authorize using a connector-issued payment method token."""
        return self._execute_flow("token_authorize", request, payment_pb2.PaymentServiceAuthorizeResponse, options)

    def token_setup_recurring(self, request, options=None):
        """PaymentService.TokenSetupRecurring — Setup a recurring mandate using a connector token."""
        return self._execute_flow("token_setup_recurring", request, payment_pb2.PaymentServiceSetupRecurringResponse, options)

    def void(self, request, options=None):
        """PaymentService.Void — Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed."""
        return self._execute_flow("void", request, payment_pb2.PaymentServiceVoidResponse, options)

    def verify_redirect_response(self, request, options=None):
        """PaymentService.VerifyRedirectResponse — Verify and process redirect responses from 3D Secure or other external flows. Validates authentication results and updates payment state accordingly."""
        return self._execute_direct("verify_redirect_response", request, payment_pb2.PaymentServiceVerifyRedirectResponseResponse, options)

class PayoutClient(_ConnectorClientBase):
    """PayoutService flows"""

    def payout_create(self, request, options=None):
        """PayoutService.Create — Creates a payout."""
        return self._execute_flow("payout_create", request, payouts_pb2.PayoutServiceCreateResponse, options)

    def payout_create_link(self, request, options=None):
        """PayoutService.CreateLink — Creates a link between the recipient and the payout."""
        return self._execute_flow("payout_create_link", request, payouts_pb2.PayoutServiceCreateLinkResponse, options)

    def payout_create_recipient(self, request, options=None):
        """PayoutService.CreateRecipient — Create payout recipient."""
        return self._execute_flow("payout_create_recipient", request, payouts_pb2.PayoutServiceCreateRecipientResponse, options)

    def payout_eligibility(self, request, options=None):
        """PayoutService.Eligibility — Check eligibility of a payout before initiating it (e.g. SEPA VoP / payee verification)."""
        return self._execute_flow("payout_eligibility", request, payouts_pb2.PayoutMethodEligibilityResponse, options)

    def payout_enroll_disburse_account(self, request, options=None):
        """PayoutService.EnrollDisburseAccount — Enroll disburse account."""
        return self._execute_flow("payout_enroll_disburse_account", request, payouts_pb2.PayoutServiceEnrollDisburseAccountResponse, options)

    def payout_get(self, request, options=None):
        """PayoutService.Get — Retrieve payout details."""
        return self._execute_flow("payout_get", request, payouts_pb2.PayoutServiceGetResponse, options)

    def payout_stage(self, request, options=None):
        """PayoutService.Stage — Stage the payout."""
        return self._execute_flow("payout_stage", request, payouts_pb2.PayoutServiceStageResponse, options)

    def payout_transfer(self, request, options=None):
        """PayoutService.Transfer — Creates a payout fund transfer."""
        return self._execute_flow("payout_transfer", request, payouts_pb2.PayoutServiceTransferResponse, options)

    def payout_void(self, request, options=None):
        """PayoutService.Void — Void a payout."""
        return self._execute_flow("payout_void", request, payouts_pb2.PayoutServiceVoidResponse, options)

class RecurringPaymentClient(_ConnectorClientBase):
    """RecurringPaymentService flows"""

    def charge(self, request, options=None):
        """RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details."""
        return self._execute_flow("charge", request, payment_pb2.RecurringPaymentServiceChargeResponse, options)

    def recurring_revoke(self, request, options=None):
        """RecurringPaymentService.Revoke — Cancel an existing recurring payment mandate. Stops future automatic charges on customer's stored consent for subscription cancellations."""
        return self._execute_flow("recurring_revoke", request, payment_pb2.RecurringPaymentServiceRevokeResponse, options)

class RefundClient(_ConnectorClientBase):
    """RefundService flows"""

    def refund_get(self, request, options=None):
        """RefundService.Get — Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication."""
        return self._execute_flow("refund_get", request, payment_pb2.RefundResponse, options)

class SurchargeClient(_ConnectorClientBase):
    """SurchargeService flows"""

    def surcharge_calculate(self, request, options=None):
        """SurchargeService.Calculate — Calculate surcharge fees for a payment amount before processing."""
        return self._execute_flow("surcharge_calculate", request, surcharge_pb2.SurchargeServiceCalculateResponse, options)
