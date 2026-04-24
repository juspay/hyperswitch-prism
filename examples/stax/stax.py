# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py stax
#
# Stax — all integration scenarios and flows in one file.
# Run a scenario:  python3 stax.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import CustomerClient
from payments import RefundClient
from payments import PaymentMethodClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["capture", "create_customer", "get", "refund", "refund_get", "token_authorize", "tokenize", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        stax=payment_pb2.StaxConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            base_url="YOUR_BASE_URL",
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
        customer=payment_pb2.Customer(  # Customer Information.
            id="cust_probe_123",  # Internal customer ID.
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


async def process_token_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.TokenAuthorize"""
    payment_client = PaymentClient(config)

    token_response = await payment_client.token_authorize(_build_token_authorize_request())

    return {"status": token_response.status}


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
