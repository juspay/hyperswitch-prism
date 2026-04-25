# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py ppro
#
# Ppro — all integration scenarios and flows in one file.
# Run a scenario:  python3 ppro.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import MerchantAuthenticationClient
from payments import EventClient
from payments import RecurringPaymentClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["authorize", "capture", "create_client_authentication_token", "get", "parse_event", "recurring_charge", "refund", "refund_get", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        ppro=payment_pb2.PproConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
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
            ideal=payment_methods_pb2.Ideal(),
        ),
        capture_method=payment_pb2.CaptureMethod.Value(capture_method),  # Method for capturing the payment.
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=payment_pb2.Address(),
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
            headers=payment_pb2.HeadersEntry(),  # Headers of the HTTP request.
            body="{\"specversion\":\"1.0\",\"type\":\"PAYMENT_CHARGE_SUCCESS\",\"source\":\"probe_source\",\"id\":\"probe_event_001\",\"time\":\"2024-01-01T00:00:00Z\",\"data\":{\"charge\":{\"id\":\"probe_txn_001\",\"status\":\"SUCCEEDED\",\"amount\":1000,\"currency\":\"EUR\"}}}",  # Body of the HTTP request.
        ),
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
        refund_id="probe_refund_id_001",  # Deprecated.
    )

def _build_void_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceVoidRequest(
        merchant_void_id="probe_void_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
    )
async def process_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Authorize (Ideal)"""
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


async def process_recurring_charge(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RecurringPaymentService.Charge"""
    recurringpayment_client = RecurringPaymentClient(config)

    recurring_response = await recurringpayment_client.charge(_build_recurring_charge_request())

    return {"status": recurring_response.status}


async def process_refund(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Refund"""
    payment_client = PaymentClient(config)

    refund_response = await payment_client.refund(_build_refund_request("probe_connector_txn_001"))

    return {"status": refund_response.status}


async def process_refund_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RefundService.Get"""
    refund_client = RefundClient(config)

    refund_response = await refund_client.refund_get(_build_refund_get_request())

    return {"status": refund_response.status}


async def process_void(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Void"""
    payment_client = PaymentClient(config)

    void_response = await payment_client.void(_build_void_request("probe_connector_txn_001"))

    return {"status": void_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "authorize"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
