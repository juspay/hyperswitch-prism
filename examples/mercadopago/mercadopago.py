# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py mercadopago
#
# Mercadopago — all integration scenarios and flows in one file.
# Run a scenario:  python3 mercadopago.py checkout_card

import asyncio
import sys
from payments import MerchantAuthenticationClient
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["create_server_session_authentication_token", "token_authorize"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        mercadopago=payment_pb2.MercadopagoConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            base_url="YOUR_BASE_URL",
        ),
    ),
)




def _build_create_server_session_authentication_token_request():
    return payment_pb2.MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest(
        payment=payment_pb2.PaymentSessionContext(  # PayoutSessionContext payout = 6; // future FrmSessionContext frm = 7; // future.
            amount=payment_pb2.Money(
                minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
                currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
            ),
        ),
    )

def _build_token_authorize_request():
    return payment_pb2.PaymentServiceTokenAuthorizeRequest(
        merchant_transaction_id="probe_tokenized_txn_001",
        amount=payment_pb2.Money(
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        connector_token=payment_methods_pb2.SecretString(value="pm_1AbcXyzStripeTestToken"),  # Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
        address=payment_pb2.PaymentAddress(
            billing_address=payment_pb2.Address(),
        ),
        capture_method=payment_pb2.CaptureMethod.Value("AUTOMATIC"),
        return_url="https://example.com/return",
    )
async def process_create_server_session_authentication_token(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: MerchantAuthenticationService.CreateServerSessionAuthenticationToken"""
    merchantauthentication_client = MerchantAuthenticationClient(config)

    create_response = await merchantauthentication_client.create_server_session_authentication_token(_build_create_server_session_authentication_token_request())

    return {"status": create_response.status}


async def process_token_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.TokenAuthorize"""
    payment_client = PaymentClient(config)

    token_response = await payment_client.token_authorize(_build_token_authorize_request())

    return {"status": token_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "create_server_session_authentication_token"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
