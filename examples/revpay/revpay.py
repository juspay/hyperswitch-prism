# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py revpay
#
# Revpay — all integration scenarios and flows in one file.
# Run a scenario:  python3 revpay.py checkout_card

import asyncio
import sys
from payments import EventClient
from payments.generated import events_pb2, payment_methods_pb2, payment_pb2, sdk_config_pb2

SUPPORTED_FLOWS = ["parse_event"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        revpay=payment_pb2.RevpayConfig(
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            merchant_key=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_KEY"),
            key_index=payment_methods_pb2.SecretString(value="YOUR_KEY_INDEX"),
            base_url="YOUR_BASE_URL",
        ),
    ),
)




def _build_parse_event_request():
    return events_pb2.EventServiceParseRequest(
        request_details=payment_pb2.RequestDetails(
            method=payment_pb2.HttpMethod.Value("HTTP_METHOD_POST"),  # HTTP method of the request (e.g., GET, POST).
            uri="https://example.com/webhook",  # URI of the request.
            headers={},  # Headers of the HTTP request.
            body="Revpay_Merchant_ID=MER00000000001&Payment_ID=2&Bank_Code=MB2U&Transaction_ID=RT000000007222&Reference_Number=RF201774111&Bank_Reference_Number=BR000001&Amount=52.40&Currency=MYR&Transaction_Description=Order+RF201774111&Fraud_Score=0&Response_Code=00&Error_Description=&Key_Index=1&Signature=fc8d556ae2e4fa12836e1192a4f0a793a9672af1d2348da502ef299a6b5169f4402fb6bb282a9e097fe1253b4bfd4f2fa365b7d593b564c585fc79b6c302ecad&Request_Datetime=2020-01-01+12%3A00%3A00&Response_Datetime=2020-01-01+12%3A00%3A05".encode(),  # Body of the HTTP request.
        ),
    )
async def process_parse_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: EventService.ParseEvent"""
    event_client = EventClient(config)

    parse_response = event_client.parse_event(_build_parse_event_request())

    return {"event_type": parse_response.event_type}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "parse_event"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
