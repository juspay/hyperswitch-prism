# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py iatapay
#
# Iatapay — all integration scenarios and flows in one file.
# Run a scenario:  python3 iatapay.py checkout_card

import asyncio
import sys
from google.protobuf.json_format import ParseDict
from payments import PaymentClient
from payments import MerchantAuthenticationClient
from payments import FraudClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Standalone credentials (field names depend on connector auth type):
# _default_config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     iatapay=payment_pb2.IatapayConfig(api_key=...),
# ))




def _build_authorize_request(capture_method: str):
    return ParseDict(
        {
            "merchant_transaction_id": "probe_txn_001",  # Identification.
            "amount": {  # The amount for the payment.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "payment_method": {  # Payment method to be used.
                "ideal": {
                }
            },
            "capture_method": capture_method,  # Method for capturing the payment.
            "address": {  # Address Information.
                "billing_address": {
                }
            },
            "auth_type": "NO_THREE_DS",  # Authentication Details.
            "return_url": "https://example.com/return",  # URLs for Redirection and Webhooks.
            "webhook_url": "https://example.com/webhook",
            "state": {  # State Information.
                "access_token": {  # Access token obtained from connector.
                    "token": {"value": "probe_access_token"},  # The token string.
                    "expires_in_seconds": 3600,  # Expiration timestamp (seconds since epoch).
                    "token_type": "Bearer"  # Token type (e.g., "Bearer", "Basic").
                }
            }
        },
        payment_pb2.PaymentServiceAuthorizeRequest(),
    )

def _build_create_server_authentication_token_request():
    return ParseDict(
        {
        },
        payment_pb2.MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest(),
    )

def _build_get_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_transaction_id": "probe_merchant_txn_001",
            "connector_transaction_id": connector_transaction_id,
            "amount": {
                "minor_amount": 1000,
                "currency": "USD"
            },
            "state": {
                "token": "probe_access_token",
                "expires_in_seconds": 3600,
                "token_type": "Bearer"
            },
            "connector_order_reference_id": "probe_order_ref_001"
        },
        payment_pb2.FraudServiceGetRequest(),
    )

def _build_refund_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_refund_id": "probe_refund_001",  # Identification.
            "connector_transaction_id": connector_transaction_id,
            "payment_amount": 1000,  # Amount Information.
            "refund_amount": {
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "reason": "customer_request",  # Reason for the refund.
            "webhook_url": "https://example.com/webhook",  # URL for webhook notifications.
            "state": {  # State data for access token storage and.
                "access_token": {  # Access token obtained from connector.
                    "token": {"value": "probe_access_token"},  # The token string.
                    "expires_in_seconds": 3600,  # Expiration timestamp (seconds since epoch).
                    "token_type": "Bearer"  # Token type (e.g., "Bearer", "Basic").
                }
            }
        },
        payment_pb2.PaymentServiceRefundRequest(),
    )

def _build_refund_get_request():
    return ParseDict(
        {
            "merchant_refund_id": "probe_refund_001",  # Identification.
            "connector_transaction_id": "probe_connector_txn_001",
            "refund_id": "probe_refund_id_001",
            "state": {  # State Information.
                "access_token": {  # Access token obtained from connector.
                    "token": {"value": "probe_access_token"},  # The token string.
                    "expires_in_seconds": 3600,  # Expiration timestamp (seconds since epoch).
                    "token_type": "Bearer"  # Token type (e.g., "Bearer", "Basic").
                }
            }
        },
        payment_pb2.RefundServiceGetRequest(),
    )
async def authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Authorize (Ideal)"""
    payment_client = PaymentClient(config)

    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}


async def create_server_authentication_token(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: MerchantAuthenticationService.CreateServerAuthenticationToken"""
    merchantauthentication_client = MerchantAuthenticationClient(config)

    create_response = await merchantauthentication_client.create_server_authentication_token(_build_create_server_authentication_token_request())

    return {"status": create_response.status}


async def get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: FraudService.Get"""
    fraud_client = FraudClient(config)

    get_response = await fraud_client.get(_build_get_request("probe_connector_txn_001"))

    return {"status": get_response.status}


async def refund(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Refund"""
    payment_client = PaymentClient(config)

    refund_response = await payment_client.refund(_build_refund_request("probe_connector_txn_001"))

    return {"status": refund_response.status}


async def refund_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RefundService.Get"""
    refund_client = RefundClient(config)

    refund_response = await refund_client.refund_get(_build_refund_get_request())

    return {"status": refund_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "authorize"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
