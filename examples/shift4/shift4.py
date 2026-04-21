# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py shift4
#
# Shift4 — all integration scenarios and flows in one file.
# Run a scenario:  python3 shift4.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import MerchantAuthenticationClient
from payments import CustomerClient
from payments import RecurringPaymentClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["authorize", "capture", "create_client_authentication_token", "create_customer", "get", "incremental_authorization", "proxy_authorize", "proxy_setup_recurring", "recurring_charge", "refund", "refund_get", "setup_recurring", "token_authorize", "token_setup_recurring"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        shift4=payment_pb2.Shift4Config(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            base_url="YOUR_BASE_URL",
        ),
    ),
)




def _build_authorize_request(capture_method: str):
    return payment_pb2.PaymentServiceAuthorizeRequest(
        merchant_transaction_id="probe_txn_001",  # Identification.
        amount=payment_pb2.Money(  # The amount for the payment.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        payment_method=payment_methods_pb2.PaymentMethod(  # Payment method to be used.
            card=payment_methods_pb2.CardDetails(
                card_number=payment_methods_pb2.CardNumberType(value="4111111111111111"),  # Card Identification.
                card_exp_month=payment_methods_pb2.SecretString(value="03"),
                card_exp_year=payment_methods_pb2.SecretString(value="2030"),
                card_cvc=payment_methods_pb2.SecretString(value="737"),
                card_holder_name=payment_methods_pb2.SecretString(value="John Doe"),  # Cardholder Information.
            ),
        ),
        capture_method=payment_pb2.CaptureMethod.Value(capture_method),  # Method for capturing the payment.
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=payment_pb2.Address(
                first_name=payment_methods_pb2.SecretString(value="John"),  # Personal Information.
            ),
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),  # Authentication Details.
        return_url="https://example.com/return",  # URLs for Redirection and Webhooks.
    )

def _build_capture_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceCaptureRequest(
        merchant_capture_id="probe_capture_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        amount_to_capture=payment_pb2.Money(  # Capture Details.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
    )

def _build_create_client_authentication_token_request():
    return payment_pb2.MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest(
        merchant_client_session_id="probe_sdk_session_001",  # Infrastructure.
        payment=payment_pb2.PaymentClientAuthenticationContext(  # FrmClientAuthenticationContext frm = 5; // future: device fingerprinting PayoutClientAuthenticationContext payout = 6; // future: payout verification widget.
            amount=payment_pb2.Money(
                minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
                currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
            ),
        ),
    )

def _build_create_customer_request():
    return payment_pb2.CustomerServiceCreateRequest(
        merchant_customer_id="cust_probe_123",  # Identification.
        customer_name="John Doe",  # Name of the customer.
        email=payment_methods_pb2.SecretString(value="test@example.com"),  # Email address of the customer.
        phone_number="4155552671",  # Phone number of the customer.
    )

def _build_get_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceGetRequest(
        merchant_transaction_id="probe_merchant_txn_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
    )

def _build_incremental_authorization_request():
    return payment_pb2.PaymentServiceIncrementalAuthorizationRequest(
        merchant_authorization_id="probe_auth_001",  # Identification.
        connector_transaction_id="probe_connector_txn_001",
        amount=payment_pb2.Money(  # new amount to be authorized (in minor currency units).
            minor_amount=1100,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        reason="incremental_auth_probe",  # Optional Fields.
    )

def _build_proxy_authorize_request():
    return payment_pb2.PaymentServiceProxyAuthorizeRequest(
        merchant_transaction_id="probe_proxy_txn_001",
        amount=payment_pb2.Money(
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        card_proxy=payment_methods_pb2.ProxyCardDetails(  # Card proxy for vault-aliased payments (VGS, Basis Theory, Spreedly). Real card values are substituted by the proxy before reaching the connector.
            card_number=payment_methods_pb2.SecretString(value="4111111111111111"),  # Card Identification.
            card_exp_month=payment_methods_pb2.SecretString(value="03"),
            card_exp_year=payment_methods_pb2.SecretString(value="2030"),
            card_cvc=payment_methods_pb2.SecretString(value="123"),
            card_holder_name=payment_methods_pb2.SecretString(value="John Doe"),  # Cardholder Information.
        ),
        address=payment_pb2.PaymentAddress(
            billing_address=payment_pb2.Address(
                first_name=payment_methods_pb2.SecretString(value="John"),  # Personal Information.
            ),
        ),
        capture_method=payment_pb2.CaptureMethod.Value("AUTOMATIC"),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),
        return_url="https://example.com/return",
    )

def _build_proxy_setup_recurring_request():
    return payment_pb2.PaymentServiceProxySetupRecurringRequest(
        merchant_recurring_payment_id="probe_proxy_mandate_001",
        amount=payment_pb2.Money(
            minor_amount=0,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        card_proxy=payment_methods_pb2.ProxyCardDetails(  # Card proxy for vault-aliased payments.
            card_number=payment_methods_pb2.SecretString(value="4111111111111111"),  # Card Identification.
            card_exp_month=payment_methods_pb2.SecretString(value="03"),
            card_exp_year=payment_methods_pb2.SecretString(value="2030"),
            card_cvc=payment_methods_pb2.SecretString(value="123"),
            card_holder_name=payment_methods_pb2.SecretString(value="John Doe"),  # Cardholder Information.
        ),
        address=payment_pb2.PaymentAddress(
            billing_address=payment_pb2.Address(
                first_name=payment_methods_pb2.SecretString(value="John"),  # Personal Information.
            ),
        ),
        customer_acceptance=payment_pb2.CustomerAcceptance(
            acceptance_type=payment_pb2.AcceptanceType.Value("OFFLINE"),  # Type of acceptance (e.g., online, offline).
            accepted_at=0,  # Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),
        setup_future_usage=payment_pb2.FutureUsage.Value("OFF_SESSION"),
    )

def _build_recurring_charge_request():
    return payment_pb2.RecurringPaymentServiceChargeRequest(
        connector_recurring_payment_id=payment_pb2.MandateReference(  # Reference to existing mandate.
            connector_mandate_id=payment_pb2.ConnectorMandateReferenceId(  # mandate_id sent by the connector.
                connector_mandate_id="probe-mandate-123",
            ),
        ),
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        payment_method=payment_methods_pb2.PaymentMethod(  # Optional payment Method Information (for network transaction flows).
            token=payment_methods_pb2.TokenPaymentMethodType(
                token=payment_methods_pb2.SecretString(value="probe_pm_token"),  # The token string representing a payment method.
            ),
        ),
        return_url="https://example.com/recurring-return",
        connector_customer_id="cust_probe_123",
        payment_method_type=payment_pb2.PaymentMethodType.Value("PAY_PAL"),
        off_session=True,  # Behavioral Flags and Preferences.
    )

def _build_refund_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceRefundRequest(
        merchant_refund_id="probe_refund_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        payment_amount=1000,  # Amount Information.
        refund_amount=payment_pb2.Money(
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        reason="customer_request",  # Reason for the refund.
    )

def _build_refund_get_request():
    return payment_pb2.RefundServiceGetRequest(
        merchant_refund_id="probe_refund_001",  # Identification.
        connector_transaction_id="probe_connector_txn_001",
        refund_id="probe_refund_id_001",
    )

def _build_setup_recurring_request():
    return payment_pb2.PaymentServiceSetupRecurringRequest(
        merchant_recurring_payment_id="probe_mandate_001",  # Identification.
        amount=payment_pb2.Money(  # Mandate Details.
            minor_amount=0,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        payment_method=payment_methods_pb2.PaymentMethod(
            card=payment_methods_pb2.CardDetails(
                card_number=payment_methods_pb2.CardNumberType(value="4111111111111111"),  # Card Identification.
                card_exp_month=payment_methods_pb2.SecretString(value="03"),
                card_exp_year=payment_methods_pb2.SecretString(value="2030"),
                card_cvc=payment_methods_pb2.SecretString(value="737"),
                card_holder_name=payment_methods_pb2.SecretString(value="John Doe"),  # Cardholder Information.
            ),
        ),
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=payment_pb2.Address(
                first_name=payment_methods_pb2.SecretString(value="John"),  # Personal Information.
            ),
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),  # Type of authentication to be used.
        enrolled_for_3ds=False,  # Indicates if the customer is enrolled for 3D Secure.
        return_url="https://example.com/mandate-return",  # URL to redirect after setup.
        setup_future_usage=payment_pb2.FutureUsage.Value("OFF_SESSION"),  # Indicates future usage intention.
        request_incremental_authorization=False,  # Indicates if incremental authorization is requested.
        customer_acceptance=payment_pb2.CustomerAcceptance(  # Details of customer acceptance.
            acceptance_type=payment_pb2.AcceptanceType.Value("OFFLINE"),  # Type of acceptance (e.g., online, offline).
            accepted_at=0,  # Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        ),
    )

def _build_token_authorize_request():
    return payment_pb2.PaymentServiceTokenAuthorizeRequest(
        merchant_transaction_id="probe_tokenized_txn_001",
        amount=payment_pb2.Money(
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        connector_token=payment_methods_pb2.SecretString(value="pm_1AbcXyzStripeTestToken"),  # Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
        address=payment_pb2.PaymentAddress(
            billing_address=payment_pb2.Address(),
        ),
        capture_method=payment_pb2.CaptureMethod.Value("AUTOMATIC"),
        return_url="https://example.com/return",
    )

def _build_token_setup_recurring_request():
    return payment_pb2.PaymentServiceTokenSetupRecurringRequest(
        merchant_recurring_payment_id="probe_tokenized_mandate_001",
        amount=payment_pb2.Money(
            minor_amount=0,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        connector_token=payment_methods_pb2.SecretString(value="pm_1AbcXyzStripeTestToken"),
        address=payment_pb2.PaymentAddress(
            billing_address=payment_pb2.Address(),
        ),
        customer_acceptance=payment_pb2.CustomerAcceptance(
            acceptance_type=payment_pb2.AcceptanceType.Value("ONLINE"),  # Type of acceptance (e.g., online, offline).
            accepted_at=0,  # Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
            online_mandate_details=payment_pb2.OnlineMandate(  # Details if the acceptance was an online mandate.
                ip_address="127.0.0.1",  # IP address from which the mandate was accepted.
                user_agent="Mozilla/5.0",  # User agent string of the browser used for mandate acceptance.
            ),
        ),
        setup_mandate_details=payment_pb2.SetupMandateDetails(
            mandate_type=payment_pb2.MandateType(  # Type of mandate (single_use or multi_use) with amount details.
                multi_use=payment_pb2.MandateAmountData(
                    amount=0,  # Amount.
                    currency=payment_pb2.Currency.Value("USD"),  # Currency code (ISO 4217).
                ),
            ),
        ),
        setup_future_usage=payment_pb2.FutureUsage.Value("OFF_SESSION"),
    )
async def process_checkout_autocapture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """One-step Payment (Authorize + Capture)

    Simple payment that authorizes and captures in one call. Use for immediate charges.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    return {"status": getattr(authorize_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(authorize_response, "error", None)}


async def process_checkout_card(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Card Payment (Authorize + Capture)

    Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("MANUAL"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Capture — settle the reserved funds
    capture_response = await payment_client.capture(_build_capture_request(authorize_response.connector_transaction_id))

    if capture_response.status == "FAILED":
        raise RuntimeError(f"Capture failed: {capture_response.error}")

    return {"status": getattr(capture_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(capture_response, "error", None)}


async def process_refund(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Refund

    Return funds to the customer for a completed payment.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Refund — return funds to the customer
    refund_response = await payment_client.refund(_build_refund_request(authorize_response.connector_transaction_id))

    if refund_response.status == "FAILED":
        raise RuntimeError(f"Refund failed: {refund_response.error}")

    return {"status": getattr(refund_response, "status", ""), "error": getattr(refund_response, "error", None)}


async def process_get_payment(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Get Payment Status

    Retrieve current payment status from the connector.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("MANUAL"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Get — retrieve current payment status from the connector
    get_response = await payment_client.get(_build_get_request(authorize_response.connector_transaction_id))

    return {"status": getattr(get_response, "status", ""), "transaction_id": getattr(get_response, "connector_transaction_id", ""), "error": getattr(get_response, "error", None)}


async def process_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Authorize (Card)"""
    payment_client = PaymentClient(config)

    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}


async def process_capture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Capture"""
    payment_client = PaymentClient(config)

    capture_response = await payment_client.capture(_build_capture_request("probe_connector_txn_001"))

    return {"status": capture_response.status}


async def process_create_client_authentication_token(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: MerchantAuthenticationService.CreateClientAuthenticationToken"""
    merchantauthentication_client = MerchantAuthenticationClient(config)

    create_response = await merchantauthentication_client.create_client_authentication_token(_build_create_client_authentication_token_request())

    return {"session_data": create_response.session_data}


async def process_create_customer(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: CustomerService.Create"""
    customer_client = CustomerClient(config)

    create_response = await customer_client.create(_build_create_customer_request())

    return {"customer_id": create_response.connector_customer_id}


async def process_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Get"""
    payment_client = PaymentClient(config)

    get_response = await payment_client.get(_build_get_request("probe_connector_txn_001"))

    return {"status": get_response.status}


async def process_incremental_authorization(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.IncrementalAuthorization"""
    payment_client = PaymentClient(config)

    incremental_response = await payment_client.incremental_authorization(_build_incremental_authorization_request())

    return {"status": incremental_response.status}


async def process_proxy_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.ProxyAuthorize"""
    payment_client = PaymentClient(config)

    proxy_response = await payment_client.proxy_authorize(_build_proxy_authorize_request())

    return {"status": proxy_response.status}


async def process_proxy_setup_recurring(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.ProxySetupRecurring"""
    payment_client = PaymentClient(config)

    proxy_response = await payment_client.proxy_setup_recurring(_build_proxy_setup_recurring_request())

    return {"status": proxy_response.status}


async def process_recurring_charge(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RecurringPaymentService.Charge"""
    recurringpayment_client = RecurringPaymentClient(config)

    recurring_response = await recurringpayment_client.charge(_build_recurring_charge_request())

    return {"status": recurring_response.status}


async def process_refund_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RefundService.Get"""
    refund_client = RefundClient(config)

    refund_response = await refund_client.refund_get(_build_refund_get_request())

    return {"status": refund_response.status}


async def process_setup_recurring(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.SetupRecurring"""
    payment_client = PaymentClient(config)

    setup_response = await payment_client.setup_recurring(_build_setup_recurring_request())

    return {"status": setup_response.status, "mandate_id": setup_response.connector_recurring_payment_id}


async def process_token_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.TokenAuthorize"""
    payment_client = PaymentClient(config)

    token_response = await payment_client.token_authorize(_build_token_authorize_request())

    return {"status": token_response.status}


async def process_token_setup_recurring(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.TokenSetupRecurring"""
    payment_client = PaymentClient(config)

    token_response = await payment_client.token_setup_recurring(_build_token_setup_recurring_request())

    return {"status": token_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "checkout_autocapture"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
