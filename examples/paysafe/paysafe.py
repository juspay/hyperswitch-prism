# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py paysafe
#
# Paysafe — all integration scenarios and flows in one file.
# Run a scenario:  python3 paysafe.py checkout_card

import asyncio
import sys
from google.protobuf.json_format import ParseDict
from payments import PaymentClient
from payments import RefundClient
from payments import PaymentMethodClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Standalone credentials (field names depend on connector auth type):
# _default_config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     paysafe=payment_pb2.PaysafeConfig(api_key=...),
# ))




def _build_capture_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_capture_id": "probe_capture_001",  # Identification.
            "connector_transaction_id": connector_transaction_id,
            "amount_to_capture": {  # Capture Details.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            }
        },
        payment_pb2.PaymentServiceCaptureRequest(),
    )

def _build_create_order_request():
    return ParseDict(
        {
            "merchant_order_id": "probe_order_001",  # Identification.
            "amount": {  # Amount Information.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            }
        },
        payment_pb2.PaymentServiceCreateOrderRequest(),
    )

def _build_get_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_transaction_id": "probe_merchant_txn_001",  # Identification.
            "connector_transaction_id": connector_transaction_id,
            "amount": {  # Amount Information.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            }
        },
        payment_pb2.PaymentServiceGetRequest(),
    )

def _build_refund_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_refund_id": "probe_refund_001",  # Identification.
            "connector_transaction_id": connector_transaction_id,
            "payment_amount": 1000,  # Amount Information.
            "refund_amount": {
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "reason": "customer_request"  # Reason for the refund.
        },
        payment_pb2.PaymentServiceRefundRequest(),
    )

def _build_refund_get_request():
    return ParseDict(
        {
            "merchant_refund_id": "probe_refund_001",  # Identification.
            "connector_transaction_id": "probe_connector_txn_001",
            "refund_id": "probe_refund_id_001"
        },
        payment_pb2.RefundServiceGetRequest(),
    )

def _build_token_authorize_request():
    return ParseDict(
        {
            "merchant_transaction_id": "probe_tokenized_txn_001",
            "amount": {
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "connector_token": {"value": "pm_1AbcXyzStripeTestToken"},  # Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
            "address": {
                "billing_address": {
                }
            },
            "capture_method": "AUTOMATIC",
            "return_url": "https://example.com/return"
        },
        payment_pb2.PaymentServiceTokenAuthorizeRequest(),
    )

def _build_tokenize_request():
    return ParseDict(
        {
            "amount": {  # Payment Information.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "payment_method": {
                "card": {  # Generic card payment.
                    "card_number": {"value": "4111111111111111"},  # Card Identification.
                    "card_exp_month": {"value": "03"},
                    "card_exp_year": {"value": "2030"},
                    "card_cvc": {"value": "737"},
                    "card_holder_name": {"value": "John Doe"}  # Cardholder Information.
                }
            },
            "address": {  # Address Information.
                "billing_address": {
                }
            },
            "return_url": "https://example.com/return"  # URLs for Redirection.
        },
        payment_pb2.PaymentMethodServiceTokenizeRequest(),
    )

def _build_void_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_void_id": "probe_void_001",  # Identification.
            "connector_transaction_id": connector_transaction_id,
            "amount": {  # Amount Information.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            }
        },
        payment_pb2.PaymentServiceVoidRequest(),
    )
async def capture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Capture"""
    payment_client = PaymentClient(config)

    capture_response = await payment_client.capture(_build_capture_request("probe_connector_txn_001"))

    return {"status": capture_response.status}


async def create_order(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.CreateOrder"""
    payment_client = PaymentClient(config)

    create_response = await payment_client.create_order(_build_create_order_request())

    return {"status": create_response.status}


async def get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Get"""
    payment_client = PaymentClient(config)

    get_response = await payment_client.get(_build_get_request("probe_connector_txn_001"))

    return {"status": get_response.status}


async def refund(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Refund"""
    payment_client = PaymentClient(config)

    refund_response = await payment_client.refund(_build_refund_request("probe_connector_txn_001"))

    return {"status": refund_response.status}


async def refund_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RefundService.Get"""
    refund_client = RefundClient(config)

    refund_response = await refund_client.refund_get(_build_refund_get_request())

    return {"status": refund_response.status}


async def token_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.TokenAuthorize"""
    payment_client = PaymentClient(config)

    token_response = await payment_client.token_authorize(_build_token_authorize_request())

    return {"status": token_response.status}


async def tokenize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentMethodService.Tokenize"""
    paymentmethod_client = PaymentMethodClient(config)

    tokenize_response = await paymentmethod_client.tokenize(_build_tokenize_request())

    return {"status": tokenize_response.status}


async def void(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
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
