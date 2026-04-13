# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py trustly
#
# Trustly — all integration scenarios and flows in one file.
# Run a scenario:  python3 trustly.py checkout_card

import asyncio
import sys
from payments import EventClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = []

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        trustly=payment_pb2.TrustlyConfig(
            username=payment_methods_pb2.SecretString(value="YOUR_USERNAME"),
            password=payment_methods_pb2.SecretString(value="YOUR_PASSWORD"),
            private_key=payment_methods_pb2.SecretString(value="YOUR_PRIVATE_KEY"),
            base_url="YOUR_BASE_URL",
        ),
    ),
)



if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "checkout_autocapture"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
