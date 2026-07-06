# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py cryptopay
#
# Cryptopay — all integration scenarios and flows in one file.
# Run a scenario:  python3 cryptopay.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import EventClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["get", "parse_event"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        cryptopay=payment_pb2.CryptopayConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            api_secret=payment_methods_pb2.SecretString(value="YOUR_API_SECRET"),
            base_url="YOUR_BASE_URL",
        ),
    ),
)




def _build_get_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceGetRequest(
        merchant_transaction_id="probe_merchant_txn_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
    )

def _build_parse_event_request():
    return payment_pb2.EventServiceParseRequest(
        request_details=payment_pb2.RequestDetails(
            method=payment_pb2.HttpMethod.Value("HTTP_METHOD_POST"),  # HTTP method of the request (e.g., GET, POST).
            uri="https://example.com/webhook",  # URI of the request.
            headers=payment_pb2.HeadersEntry(),  # Headers of the HTTP request.
            body="{\"type\":\"Invoice\",\"event\":\"status_changed\",\"data\":{\"id\":\"probe_invoice_001\",\"status\":\"completed\",\"price_amount\":\"10.00\",\"price_currency\":\"USD\",\"name\":\"probe_charge\"}}",  # Body of the HTTP request.
        ),
    )
async def process_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Get"""
    payment_client = PaymentClient(config)

    get_response = await payment_client.get(_build_get_request("probe_connector_txn_001"))

    return {"status": get_response.status}


async def process_parse_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: EventService.ParseEvent"""
    event_client = EventClient(config)

    parse_response = await event_client.parse_event(_build_parse_event_request())

    return {"status": parse_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "get"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
