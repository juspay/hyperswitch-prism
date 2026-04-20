# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py itaubank
#
# Itaubank — all integration scenarios and flows in one file.
# Run a scenario:  python3 itaubank.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["create_server_authentication_token"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     itaubank=payment_pb2.ItaubankConfig(api_key=...),
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

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "create_server_authentication_token"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
