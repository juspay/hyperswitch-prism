# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py razorpay
#
# Razorpay — all integration scenarios and flows in one file.
# Run a scenario:  python3 razorpay.py checkout_card

import asyncio
import sys
from google.protobuf.json_format import ParseDict
from payments import PaymentClient
from payments import EventClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Standalone credentials (field names depend on connector auth type):
# _default_config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     razorpay=payment_pb2.RazorpayConfig(api_key=...),
# ))




def _build_authorize_request(capture_method: str):
    return ParseDict(
        {
            "merchant_transaction_id": "probe_txn_001",  # Identification.
            "amount": {  # The amount for the payment.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "payment_method": {  # Payment method to be used.
                "upi_collect": {  # UPI Collect.
                    "vpa_id": {"value": "test@upi"}  # Virtual Payment Address.
                }
            },
            "capture_method": capture_method,  # Method for capturing the payment.
            "address": {  # Address Information.
                "billing_address": {
                }
            },
            "auth_type": "NO_THREE_DS",  # Authentication Details.
            "return_url": "https://example.com/return",  # URLs for Redirection and Webhooks.
            "merchant_order_id": "probe_order_001"
        },
        payment_pb2.PaymentServiceAuthorizeRequest(),
    )

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

def _build_handle_event_request():
    return ParseDict(
        {
        },
        payment_pb2.EventServiceHandleRequest(),
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
async def authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Authorize (UpiCollect)"""
    payment_client = PaymentClient(config)

    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}


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


async def handle_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: EventService.HandleEvent"""
    event_client = EventClient(config)

    handle_response = await event_client.handle_event(_build_handle_event_request())

    return {"status": handle_response.status}


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

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "authorize"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
