# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py trustly
#
# Trustly — all integration scenarios and flows in one file.
# Run a scenario:  python3 trustly.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["parse_event"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     trustly=payment_pb2.TrustlyConfig(api_key=...),
    # ),
)


async def process_parse_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.parse_event"""
    payment_client = PaymentClient(config)

    # Step 1: parse_event
    parse_response = await payment_client.parse_event(payment_pb2.TODO_FIX_MISSING_TYPE_parse_event(
        method="HTTP_METHOD_POST",
        uri="https://example.com/webhook",
        body="{\"method\":\"charge\",\"params\":{\"data\":{\"orderid\":\"probe_order_001\",\"amount\":\"10.00\",\"currency\":\"EUR\",\"enduserid\":\"probe_user\"}}}",
    ))

    return {"status": parse_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "parse_event"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
