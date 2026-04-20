# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py phonepe
#
# Phonepe — all integration scenarios and flows in one file.
# Run a scenario:  python3 phonepe.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["authorize", "get"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     phonepe=payment_pb2.PhonepeConfig(api_key=...),
    # ),
)


async def process_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.authorize (UpiCollect)"""
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(payment_pb2.TODO_FIX_MISSING_TYPE_authorize(
        merchant_transaction_id="probe_txn_001",
        minor_amount=1000,
        currency="USD",
        vpa_id="test@upi",
        capture_method="AUTOMATIC",
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
        webhook_url="https://example.com/webhook",
    ))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}


async def process_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.get"""
    payment_client = PaymentClient(config)

    # Step 1: Get — retrieve current payment status from the connector
    get_response = await payment_client.get(payment_pb2.TODO_FIX_MISSING_TYPE_get(
        merchant_transaction_id="probe_merchant_txn_001",
        connector_transaction_id="probe_connector_txn_001",
        minor_amount=1000,
        currency="USD",
        connector_order_reference_id="probe_order_ref_001",
    ))

    return {"status": get_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "authorize"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
