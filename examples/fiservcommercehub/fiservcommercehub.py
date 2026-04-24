# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py fiservcommercehub
#
# Fiservcommercehub — all integration scenarios and flows in one file.
# Run a scenario:  python3 fiservcommercehub.py checkout_card

import asyncio
import sys
from payments import MerchantAuthenticationClient
from payments import PaymentClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["create_server_authentication_token", "get", "refund", "refund_get", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        fiservcommercehub=payment_pb2.FiservcommercehubConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            secret=payment_methods_pb2.SecretString(value="YOUR_SECRET"),
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            terminal_id=payment_methods_pb2.SecretString(value="YOUR_TERMINAL_ID"),
            base_url="YOUR_BASE_URL",
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
                token=payment_methods_pb2.SecretString(value="probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA"),  # The token string.
                expires_in_seconds=3600,  # Expiration timestamp (seconds since epoch).
                token_type="Bearer",  # Token type (e.g., "Bearer", "Basic").
            ),
        ),
    )

def _build_refund_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceRefundRequest(
        merchant_refund_id="probe_refund_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        payment_amount=1000,  # Amount Information.
        refund_amount=payment_pb2.Money(
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        reason="customer_request",  # Reason for the refund.
        state=payment_pb2.ConnectorState(  # State data for access token storage and.
            access_token=payment_pb2.AccessToken(  # Access token obtained from connector.
                token=payment_methods_pb2.SecretString(value="probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA"),  # The token string.
                expires_in_seconds=3600,  # Expiration timestamp (seconds since epoch).
                token_type="Bearer",  # Token type (e.g., "Bearer", "Basic").
            ),
        ),
    )

def _build_refund_get_request():
    return payment_pb2.RefundServiceGetRequest(
        merchant_refund_id="probe_refund_001",  # Identification.
        connector_transaction_id="probe_connector_txn_001",
        refund_id="probe_refund_id_001",  # Deprecated.
        state=payment_pb2.ConnectorState(  # State Information.
            access_token=payment_pb2.AccessToken(  # Access token obtained from connector.
                token=payment_methods_pb2.SecretString(value="probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA"),  # The token string.
                expires_in_seconds=3600,  # Expiration timestamp (seconds since epoch).
                token_type="Bearer",  # Token type (e.g., "Bearer", "Basic").
            ),
        ),
    )

def _build_void_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceVoidRequest(
        merchant_void_id="probe_void_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        state=payment_pb2.ConnectorState(  # State Information.
            access_token=payment_pb2.AccessToken(  # Access token obtained from connector.
                token=payment_methods_pb2.SecretString(value="probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA"),  # The token string.
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


async def process_refund(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Refund"""
    payment_client = PaymentClient(config)

    refund_response = await payment_client.refund(_build_refund_request("probe_connector_txn_001"))

    return {"status": refund_response.status}


async def process_refund_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RefundService.Get"""
    refund_client = RefundClient(config)

    refund_response = await refund_client.refund_get(_build_refund_get_request())

    return {"status": refund_response.status}


async def process_void(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Void"""
    payment_client = PaymentClient(config)

    void_response = await payment_client.void(_build_void_request("probe_connector_txn_001"))

    return {"status": void_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "create_server_authentication_token"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
