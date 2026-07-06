# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py braintree
#
# Braintree — all integration scenarios and flows in one file.
# Run a scenario:  python3 braintree.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import MerchantAuthenticationClient
from payments import EventClient
from payments import PaymentMethodClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["capture", "create_client_authentication_token", "get", "parse_event", "proxy_setup_recurring", "refund", "reverse", "setup_recurring", "tokenize", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        braintree=payment_pb2.BraintreeConfig(
            public_key=payment_methods_pb2.SecretString(value="YOUR_PUBLIC_KEY"),
            private_key=payment_methods_pb2.SecretString(value="YOUR_PRIVATE_KEY"),
            base_url="YOUR_BASE_URL",
            merchant_account_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ACCOUNT_ID"),
            merchant_config_currency="YOUR_MERCHANT_CONFIG_CURRENCY",
            apple_pay_supported_networks=["YOUR_APPLE_PAY_SUPPORTED_NETWORKS"],
            apple_pay_merchant_capabilities=["YOUR_APPLE_PAY_MERCHANT_CAPABILITIES"],
            apple_pay_label="YOUR_APPLE_PAY_LABEL",
            gpay_merchant_name="YOUR_GPAY_MERCHANT_NAME",
            gpay_merchant_id="YOUR_GPAY_MERCHANT_ID",
            gpay_allowed_auth_methods=["YOUR_GPAY_ALLOWED_AUTH_METHODS"],
            gpay_allowed_card_networks=["YOUR_GPAY_ALLOWED_CARD_NETWORKS"],
            paypal_client_id="YOUR_PAYPAL_CLIENT_ID",
            gpay_gateway_merchant_id="YOUR_GPAY_GATEWAY_MERCHANT_ID",
        ),
    ),
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
            headers={},  # Headers of the HTTP request.
            body="bt_signature=dummy_public_key%7Cdummy_signature&bt_payload=PG5vdGlmaWNhdGlvbj48a2luZD5kaXNwdXRlX29wZW5lZDwva2luZD48dGltZXN0YW1wPjIwMjQtMDEtMDFUMDA6MDA6MDBaPC90aW1lc3RhbXA%2BPGRpc3B1dGU%2BPGFtb3VudF9kaXNwdXRlZD4xMDAwPC9hbW91bnRfZGlzcHV0ZWQ%2BPGN1cnJlbmN5X2lzb19jb2RlPlVTRDwvY3VycmVuY3lfaXNvX2NvZGU%2BPGlkPmR1bW15X2Rpc3B1dGVfaWRfMDAxPC9pZD48a2luZD5DSEFSR0VCQUNLPC9raW5kPjxzdGF0dXM%2Bb3Blbjwvc3RhdHVzPjxyZWFzb24%2BZnJhdWQ8L3JlYXNvbj48cmVhc29uX2NvZGU%2BODM8L3JlYXNvbl9jb2RlPjx0cmFuc2FjdGlvbj48YW1vdW50PjEwLjAwPC9hbW91bnQ%2BPGlkPmR1bW15X3R4bl9pZF8wMDE8L2lkPjwvdHJhbnNhY3Rpb24%2BPC9kaXNwdXRlPjwvbm90aWZpY2F0aW9uPg%3D%3D".encode(),  # Body of the HTTP request.
        ),
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
            card_network=payment_methods_pb2.CardNetwork.Value("VISA"),
        ),
        address=payment_pb2.PaymentAddress(
            billing_address=payment_pb2.Address(),
        ),
        customer_acceptance=payment_pb2.CustomerAcceptance(
            acceptance_type=payment_pb2.AcceptanceType.Value("OFFLINE"),  # Type of acceptance (e.g., online, offline).
            accepted_at=0,  # Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),
        setup_future_usage=payment_pb2.FutureUsage.Value("OFF_SESSION"),
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

def _build_reverse_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceReverseRequest(
        merchant_reverse_id="probe_reverse_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
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
            billing_address=payment_pb2.Address(),
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

def _build_tokenize_request():
    return payment_pb2.PaymentMethodServiceTokenizeRequest(
        amount=payment_pb2.Money(  # Payment Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
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
            billing_address=payment_pb2.Address(),
        ),
    )

def _build_void_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceVoidRequest(
        merchant_void_id="probe_void_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
    )
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


async def process_proxy_setup_recurring(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.ProxySetupRecurring"""
    payment_client = PaymentClient(config)

    proxy_response = await payment_client.proxy_setup_recurring(_build_proxy_setup_recurring_request())

    return {"status": proxy_response.status}


async def process_refund(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Refund"""
    payment_client = PaymentClient(config)

    refund_response = await payment_client.refund(_build_refund_request("probe_connector_txn_001"))

    return {"status": refund_response.status}


async def process_reverse(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Reverse"""
    payment_client = PaymentClient(config)

    reverse_response = await payment_client.reverse(_build_reverse_request("probe_connector_txn_001"))

    return {"status": reverse_response.status}


async def process_setup_recurring(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.SetupRecurring"""
    payment_client = PaymentClient(config)

    setup_response = await payment_client.setup_recurring(_build_setup_recurring_request())

    return {"status": setup_response.status, "mandate_id": setup_response.connector_recurring_payment_id}


async def process_tokenize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentMethodService.Tokenize"""
    paymentmethod_client = PaymentMethodClient(config)

    tokenize_response = await paymentmethod_client.tokenize(_build_tokenize_request())

    return {"token": tokenize_response.payment_method_token}


async def process_void(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Void"""
    payment_client = PaymentClient(config)

    void_response = await payment_client.void(_build_void_request("probe_connector_txn_001"))

    return {"status": void_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "capture"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
