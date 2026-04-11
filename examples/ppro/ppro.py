# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py ppro
#
# Ppro — all integration scenarios and flows in one file.
# Run a scenario:  python3 ppro.py checkout_card

import asyncio
import sys
from google.protobuf.json_format import ParseDict
from payments import PaymentClient
from payments import FraudClient
from payments import EventClient
from payments import RecurringPaymentClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Standalone credentials (field names depend on connector auth type):
# _default_config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     ppro=payment_pb2.PproConfig(api_key=...),
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
                "ideal": {
                }
            },
            "capture_method": capture_method,  # Method for capturing the payment.
            "address": {  # Address Information.
                "billing_address": {
                }
            },
            "auth_type": "NO_THREE_DS",  # Authentication Details.
            "return_url": "https://example.com/return"  # URLs for Redirection and Webhooks.
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

def _build_get_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_transaction_id": "probe_merchant_txn_001",
            "connector_transaction_id": connector_transaction_id,
            "amount": {
                "minor_amount": 1000,
                "currency": "USD"
            }
        },
        payment_pb2.FraudServiceGetRequest(),
    )

def _build_handle_event_request():
    return ParseDict(
        {
        },
        payment_pb2.EventServiceHandleRequest(),
    )

def _build_recurring_charge_request():
    return ParseDict(
        {
            "connector_recurring_payment_id": {  # Reference to existing mandate.
                "connector_mandate_id": {  # mandate_id sent by the connector.
                    "connector_mandate_id": "probe-mandate-123"
                }
            },
            "amount": {  # Amount Information.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "payment_method": {  # Optional payment Method Information (for network transaction flows).
                "token": {  # Payment tokens.
                    "token": {"value": "probe_pm_token"}  # The token string representing a payment method.
                }
            },
            "return_url": "https://example.com/recurring-return",
            "connector_customer_id": "cust_probe_123",
            "payment_method_type": "PAY_PAL",
            "off_session": True  # Behavioral Flags and Preferences.
        },
        payment_pb2.RecurringPaymentServiceChargeRequest(),
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
async def authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Authorize (Ideal)"""
    payment_client = PaymentClient(config)

    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}


async def capture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Capture"""
    payment_client = PaymentClient(config)

    capture_response = await payment_client.capture(_build_capture_request("probe_connector_txn_001"))

    return {"status": capture_response.status}


async def get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: FraudService.Get"""
    fraud_client = FraudClient(config)

    get_response = await fraud_client.get(_build_get_request("probe_connector_txn_001"))

    return {"status": get_response.status}


async def handle_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: EventService.HandleEvent"""
    event_client = EventClient(config)

    handle_response = await event_client.handle_event(_build_handle_event_request())

    return {"status": handle_response.status}


async def recurring_charge(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RecurringPaymentService.Charge"""
    recurringpayment_client = RecurringPaymentClient(config)

    recurring_response = await recurringpayment_client.charge(_build_recurring_charge_request())

    return {"status": recurring_response.status}


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


async def void(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
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
