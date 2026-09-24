# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py twoc_twop_paco
#
# Twoc_Twop_Paco — all integration scenarios and flows in one file.
# Run a scenario:  python3 twoc_twop_paco.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, events_pb2, payment_methods_pb2

SUPPORTED_FLOWS: list[str] = []

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        twoc_twop_paco=payment_pb2.TwocTwopPacoConfig(
            access_token=payment_methods_pb2.SecretString(value="YOUR_ACCESS_TOKEN"),
            office_id=payment_methods_pb2.SecretString(value="YOUR_OFFICE_ID"),
            paco_kid=payment_methods_pb2.SecretString(value="YOUR_PACO_KID"),
            merchant_signing_private_key=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_SIGNING_PRIVATE_KEY"),
            merchant_encryption_private_key=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ENCRYPTION_PRIVATE_KEY"),
            paco_signing_public_key=payment_methods_pb2.SecretString(value="YOUR_PACO_SIGNING_PUBLIC_KEY"),
            paco_encryption_public_key=payment_methods_pb2.SecretString(value="YOUR_PACO_ENCRYPTION_PUBLIC_KEY"),
            response_audience=payment_methods_pb2.SecretString(value="YOUR_RESPONSE_AUDIENCE"),
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
