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

SUPPORTED_FLOWS = ["parse_event"]

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




def _build_parse_event_request():
    return payment_pb2.EventServiceParseRequest(
        request_details=payment_pb2.RequestDetails(
            method=payment_pb2.HttpMethod.Value("HTTP_METHOD_POST"),  # HTTP method of the request (e.g., GET, POST).
            uri="https://example.com/webhook",  # URI of the request.
            headers=payment_pb2.HeadersEntry(),  # Headers of the HTTP request.
            body="{\"method\":\"charge\",\"params\":{\"data\":{\"orderid\":\"probe_order_001\",\"amount\":\"10.00\",\"currency\":\"EUR\",\"enduserid\":\"probe_user\"}}}",  # Body of the HTTP request.
        ),
    )
async def process_parse_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: EventService.ParseEvent"""
    event_client = EventClient(config)

    parse_response = await event_client.parse_event(_build_parse_event_request())

    return {"status": parse_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "parse_event"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
