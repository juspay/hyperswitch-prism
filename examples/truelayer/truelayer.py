# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py truelayer
#
# Truelayer — all integration scenarios and flows in one file.
# Run a scenario:  python3 truelayer.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["create_server_authentication_token", "get", "parse_event", "refund_get"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     truelayer=payment_pb2.TruelayerConfig(api_key=...),
    # ),
)


async def process_create_server_authentication_token(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.create_server_authentication_token"""
    payment_client = PaymentClient(config)

    # Step 1: create_server_authentication_token
    create_response = await payment_client.create_server_authentication_token(payment_pb2.TODO_FIX_MISSING_TYPE_create_server_authentication_token(
        # No required fields
    ))

    return {"status": create_response.status}


async def process_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.get"""
    payment_client = PaymentClient(config)

    # Step 1: Get — retrieve current payment status from the connector
    get_response = await payment_client.get(payment_pb2.TODO_FIX_MISSING_TYPE_get(
        merchant_transaction_id="probe_merchant_txn_001",
        connector_transaction_id="probe_connector_txn_001",
        minor_amount=1000,
        currency="USD",
        token="probe_access_token",
        expires_in_seconds=3600,
        token_type="Bearer",
    ))

    return {"status": get_response.status}


async def process_parse_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.parse_event"""
    payment_client = PaymentClient(config)

    # Step 1: parse_event
    parse_response = await payment_client.parse_event(payment_pb2.TODO_FIX_MISSING_TYPE_parse_event(
        method="HTTP_METHOD_POST",
        uri="https://example.com/webhook",
        body="{\"type\":\"payment_executed\",\"payment_id\":\"probe_payment_001\"}",
    ))

    return {"status": parse_response.status}


async def process_refund_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.refund_get"""
    payment_client = PaymentClient(config)

    # Step 1: refund_get
    refund_response = await payment_client.refund_get(payment_pb2.TODO_FIX_MISSING_TYPE_refund_get(
        merchant_refund_id="probe_refund_001",
        connector_transaction_id="probe_connector_txn_001",
        refund_id="probe_refund_id_001",
        token="probe_access_token",
        expires_in_seconds=3600,
        token_type="Bearer",
    ))

    return {"status": refund_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "create_server_authentication_token"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
