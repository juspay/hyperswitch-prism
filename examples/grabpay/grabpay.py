# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py grabpay
#
# Grabpay — all integration scenarios and flows in one file.
# Run a scenario:  python3 grabpay.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import EventClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["create_order", "parse_event"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        grabpay=payment_pb2.GrabpayConfig(
            partner_id=payment_methods_pb2.SecretString(value="YOUR_PARTNER_ID"),
            partner_secret=payment_methods_pb2.SecretString(value="YOUR_PARTNER_SECRET"),
            client_id=payment_methods_pb2.SecretString(value="YOUR_CLIENT_ID"),
            client_secret=payment_methods_pb2.SecretString(value="YOUR_CLIENT_SECRET"),
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            base_url="YOUR_BASE_URL",
        ),
    ),
)




def _build_create_order_request():
    return payment_pb2.PaymentServiceCreateOrderRequest(
        merchant_order_id="probe_order_001",  # Identification.
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
            body="{\"txType\":\"payment\",\"txStatus\":\"success\",\"partnerID\":\"partner_123\",\"partnerTxID\":\"txn_123\",\"txID\":\"grab_txn_123\",\"amount\":100,\"currency\":\"SGD\",\"payload\":{\"newStatus\":\"success\",\"paymentMethod\":\"GRABPAY\"}}".encode(),  # Body of the HTTP request.
        ),
    )
async def process_create_order(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.CreateOrder"""
    payment_client = PaymentClient(config)

    create_response = await payment_client.create_order(_build_create_order_request())

    return {"status": create_response.status}


async def process_parse_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: EventService.ParseEvent"""
    event_client = EventClient(config)

    parse_response = event_client.parse_event(_build_parse_event_request())

    return {"event_type": parse_response.event_type}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "create_order"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
