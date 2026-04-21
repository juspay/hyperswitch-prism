# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py novalnet
#
# Novalnet — all integration scenarios and flows in one file.
# Run a scenario:  python3 novalnet.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import EventClient
from payments import RecurringPaymentClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["authorize", "capture", "get", "parse_event", "proxy_authorize", "proxy_setup_recurring", "recurring_charge", "refund", "refund_get", "setup_recurring", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        novalnet=payment_pb2.NovalnetConfig(
            product_activation_key=payment_methods_pb2.SecretString(value="YOUR_PRODUCT_ACTIVATION_KEY"),
            payment_access_key=payment_methods_pb2.SecretString(value="YOUR_PAYMENT_ACCESS_KEY"),
            tariff_id=payment_methods_pb2.SecretString(value="YOUR_TARIFF_ID"),
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
        customer=payment_pb2.Customer(  # Customer Information.
            email=payment_methods_pb2.SecretString(value="test@example.com"),  # Customer's email address.
        ),
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=payment_pb2.Address(
                first_name=payment_methods_pb2.SecretString(value="John"),  # Personal Information.
            ),
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),  # Authentication Details.
        return_url="https://example.com/return",  # URLs for Redirection and Webhooks.
        webhook_url="https://example.com/webhook",
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

def _build_get_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceGetRequest(
        merchant_transaction_id="probe_merchant_txn_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
    )

def _build_parse_event_request():
    return payment_pb2.EventServiceParseRequest(
        request_details=payment_pb2.RequestDetails(
            method=payment_pb2.HttpMethod.Value("HTTP_METHOD_POST"),  # HTTP method of the request (e.g., GET, POST).
            uri="https://example.com/webhook",  # URI of the request.
            headers=payment_pb2.HeadersEntry(),  # Headers of the HTTP request.
            body="{\"event\":{\"checksum\":\"probe_checksum\",\"tid\":12345678901234,\"type\":\"PAYMENT\"},\"result\":{\"status\":\"SUCCESS\",\"status_code\":100,\"status_text\":\"Success\"},\"transaction\":{\"tid\":12345678901234,\"payment_type\":\"CREDITCARD\",\"status\":\"CONFIRMED\",\"status_code\":100,\"order_no\":\"probe_order_001\",\"amount\":1000,\"currency\":\"EUR\"}}",  # Body of the HTTP request.
        ),
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
        customer=payment_pb2.Customer(
            email=payment_methods_pb2.SecretString(value="test@example.com"),  # Customer's email address.
        ),
        address=payment_pb2.PaymentAddress(
            billing_address=payment_pb2.Address(
                first_name=payment_methods_pb2.SecretString(value="John"),  # Personal Information.
            ),
        ),
        capture_method=payment_pb2.CaptureMethod.Value("AUTOMATIC"),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),
        return_url="https://example.com/return",
        webhook_url="https://example.com/webhook",
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
        customer=payment_pb2.Customer(
            email=payment_methods_pb2.SecretString(value="test@example.com"),  # Customer's email address.
        ),
        address=payment_pb2.PaymentAddress(
            billing_address=payment_pb2.Address(
                first_name=payment_methods_pb2.SecretString(value="John"),  # Personal Information.
            ),
        ),
        return_url="https://example.com/return",
        webhook_url="https://example.com/webhook",
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
        webhook_url="https://example.com/webhook",
        return_url="https://example.com/recurring-return",
        email=payment_methods_pb2.SecretString(value="test@example.com"),  # Customer Information.
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
        refund_id="probe_refund_id_001",  # Deprecated.
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
        customer=payment_pb2.Customer(
            email=payment_methods_pb2.SecretString(value="test@example.com"),  # Customer's email address.
        ),
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=payment_pb2.Address(
                first_name=payment_methods_pb2.SecretString(value="John"),  # Personal Information.
            ),
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),  # Type of authentication to be used.
        enrolled_for_3ds=False,  # Indicates if the customer is enrolled for 3D Secure.
        return_url="https://example.com/mandate-return",  # URL to redirect after setup.
        webhook_url="https://example.com/webhook",  # URL for webhook notifications.
        setup_future_usage=payment_pb2.FutureUsage.Value("OFF_SESSION"),  # Indicates future usage intention.
        request_incremental_authorization=False,  # Indicates if incremental authorization is requested.
        customer_acceptance=payment_pb2.CustomerAcceptance(  # Details of customer acceptance.
            acceptance_type=payment_pb2.AcceptanceType.Value("OFFLINE"),  # Type of acceptance (e.g., online, offline).
            accepted_at=0,  # Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        ),
    )

def _build_void_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceVoidRequest(
        merchant_void_id="probe_void_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
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


async def process_void_payment(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Void Payment

    Cancel an authorized but not-yet-captured payment.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("MANUAL"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Void — release reserved funds (cancel authorization)
    void_response = await payment_client.void(_build_void_request(authorize_response.connector_transaction_id))

    return {"status": getattr(void_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(void_response, "error", None)}


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


async def process_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Get"""
    payment_client = PaymentClient(config)

    get_response = await payment_client.get(_build_get_request("probe_connector_txn_001"))

    return {"status": get_response.status}


async def process_parse_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: EventService.ParseEvent"""
    event_client = EventClient(config)

    parse_response = await event_client.parse_event(_build_parse_event_request())

    return {"status": parse_response.status}


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


async def process_void(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Void"""
    payment_client = PaymentClient(config)

    void_response = await payment_client.void(_build_void_request("probe_connector_txn_001"))

    return {"status": void_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "checkout_autocapture"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
