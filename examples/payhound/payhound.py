# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py payhound
#
# Payhound — all integration scenarios and flows in one file.
# Run a scenario:  python3 payhound.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import EventClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["get", "parse_event"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        payhound=payment_pb2.PayhoundConfig(
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
            headers={},  # Headers of the HTTP request.
            body="{\"id\":\"378d8ec6e305f469b009cb4e2deedf93\",\"status\":\"completed\",\"address\":\"lAeMbkpHia8FVuKczQKUrv9uMzv7uClHZi\",\"merchant_currency\":\"EUR\",\"merchant_amount\":\"266.45\",\"invoice_currency\":\"BTC\",\"invoice_amount\":\"0.88613839\",\"paid_currency\":\"BTC\",\"paid_amount\":\"0.88613839\",\"reference\":\"probe_ref\",\"invoice_url\":\"/invoices/378d8ec6e305f469b009cb4e2deedf93\",\"create_time\":1398871897.0,\"valid_until_time\":1398872497.0}".encode(),  # Body of the HTTP request.
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

    parse_response = event_client.parse_event(_build_parse_event_request())

    return {"event_type": parse_response.event_type}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "get"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
