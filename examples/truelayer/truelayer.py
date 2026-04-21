# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py truelayer
#
# Truelayer — all integration scenarios and flows in one file.
# Run a scenario:  python3 truelayer.py checkout_card

import asyncio
import sys
from payments import MerchantAuthenticationClient
from payments import PaymentClient
from payments import EventClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["create_server_authentication_token", "get", "parse_event", "refund_get"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        truelayer=payment_pb2.TruelayerConfig(
            client_id=payment_methods_pb2.SecretString(value="YOUR_CLIENT_ID"),
            client_secret=payment_methods_pb2.SecretString(value="YOUR_CLIENT_SECRET"),
            merchant_account_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ACCOUNT_ID"),
            account_holder_name=payment_methods_pb2.SecretString(value="YOUR_ACCOUNT_HOLDER_NAME"),
            private_key=payment_methods_pb2.SecretString(value="YOUR_PRIVATE_KEY"),
            kid=payment_methods_pb2.SecretString(value="YOUR_KID"),
            base_url="YOUR_BASE_URL",
            secondary_base_url="YOUR_SECONDARY_BASE_URL",
        ),
    ),
)




def _build_create_server_authentication_token_request():
    return payment_pb2.MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest(
    )

def _build_get_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceGetRequest(
        merchant_transaction_id="probe_merchant_txn_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        state=payment_pb2.ConnectorState(  # State Information.
            access_token=payment_pb2.AccessToken(  # Access token obtained from connector.
                token=payment_methods_pb2.SecretString(value="probe_access_token"),  # The token string.
                expires_in_seconds=3600,  # Expiration timestamp (seconds since epoch).
                token_type="Bearer",  # Token type (e.g., "Bearer", "Basic").
            ),
        ),
    )

def _build_parse_event_request():
    return payment_pb2.EventServiceParseRequest(
        request_details=payment_pb2.RequestDetails(
            method=payment_pb2.HttpMethod.Value("HTTP_METHOD_POST"),  # HTTP method of the request (e.g., GET, POST).
            uri="https://example.com/webhook",  # URI of the request.
            headers=payment_pb2.HeadersEntry(),  # Headers of the HTTP request.
            body="{\"type\":\"payment_executed\",\"payment_id\":\"probe_payment_001\"}",  # Body of the HTTP request.
        ),
    )

def _build_refund_get_request():
    return payment_pb2.RefundServiceGetRequest(
        merchant_refund_id="probe_refund_001",  # Identification.
        connector_transaction_id="probe_connector_txn_001",
        refund_id="probe_refund_id_001",
        state=payment_pb2.ConnectorState(  # State Information.
            access_token=payment_pb2.AccessToken(  # Access token obtained from connector.
                token=payment_methods_pb2.SecretString(value="probe_access_token"),  # The token string.
                expires_in_seconds=3600,  # Expiration timestamp (seconds since epoch).
                token_type="Bearer",  # Token type (e.g., "Bearer", "Basic").
            ),
        ),
    )
async def process_create_server_authentication_token(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: MerchantAuthenticationService.CreateServerAuthenticationToken"""
    merchantauthentication_client = MerchantAuthenticationClient(config)

    create_response = await merchantauthentication_client.create_server_authentication_token(_build_create_server_authentication_token_request())

    return {"status": create_response.status}


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


async def process_refund_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RefundService.Get"""
    refund_client = RefundClient(config)

    refund_response = await refund_client.refund_get(_build_refund_get_request())

    return {"status": refund_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "create_server_authentication_token"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
