# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py braintree
#
# Braintree — all integration scenarios and flows in one file.
# Run a scenario:  python3 braintree.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import MerchantAuthenticationClient
from payments import RefundClient
from payments import PaymentMethodClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["capture", "create_client_authentication_token", "get", "refund", "refund_get", "tokenize", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        braintree=payment_pb2.BraintreeConfig(
            public_key=payment_methods_pb2.SecretString(value="YOUR_PUBLIC_KEY"),
            private_key=payment_methods_pb2.SecretString(value="YOUR_PRIVATE_KEY"),
            base_url="YOUR_BASE_URL",
            merchant_account_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ACCOUNT_ID"),
            merchant_config_currency="YOUR_MERCHANT_CONFIG_CURRENCY",
            apple_pay_supported_networks=["YOUR_APPLE_PAY_SUPPORTED_NETWORKS"],
            apple_pay_merchant_capabilities=["YOUR_APPLE_PAY_MERCHANT_CAPABILITIES"],
            apple_pay_label="YOUR_APPLE_PAY_LABEL",
            gpay_merchant_name="YOUR_GPAY_MERCHANT_NAME",
            gpay_merchant_id="YOUR_GPAY_MERCHANT_ID",
            gpay_allowed_auth_methods=["YOUR_GPAY_ALLOWED_AUTH_METHODS"],
            gpay_allowed_card_networks=["YOUR_GPAY_ALLOWED_CARD_NETWORKS"],
            paypal_client_id="YOUR_PAYPAL_CLIENT_ID",
            gpay_gateway_merchant_id="YOUR_GPAY_GATEWAY_MERCHANT_ID",
        ),
    ),
)




def _build_capture_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceCaptureRequest(
        merchant_capture_id="probe_capture_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        amount_to_capture=payment_pb2.Money(  # Capture Details.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
    )

def _build_create_client_authentication_token_request():
    return payment_pb2.MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest(
        merchant_client_session_id="probe_sdk_session_001",  # Infrastructure.
        payment=payment_pb2.PaymentClientAuthenticationContext(  # FrmClientAuthenticationContext frm = 5; // future: device fingerprinting PayoutClientAuthenticationContext payout = 6; // future: payout verification widget.
            amount=payment_pb2.Money(
                minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
                currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
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
    )

def _build_refund_get_request():
    return payment_pb2.RefundServiceGetRequest(
        merchant_refund_id="probe_refund_001",  # Identification.
        connector_transaction_id="probe_connector_txn_001",
        refund_id="probe_refund_id_001",
        refund_metadata=payment_methods_pb2.SecretString(value="{\"currency\":\"USD\"}"),  # Metadata specific to the refund sync.
    )

def _build_tokenize_request():
    return payment_pb2.PaymentMethodServiceTokenizeRequest(
        amount=payment_pb2.Money(  # Payment Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        payment_method=payment_methods_pb2.PaymentMethod(
            card=payment_methods_pb2.CardDetails(
                card_number=payment_methods_pb2.CardNumberType(value="4111111111111111"),  # Card Identification.
                card_exp_month=payment_methods_pb2.SecretString(value="03"),
                card_exp_year=payment_methods_pb2.SecretString(value="2030"),
                card_cvc=payment_methods_pb2.SecretString(value="737"),
                card_holder_name=payment_methods_pb2.SecretString(value="John Doe"),  # Cardholder Information.
            ),
        ),
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=payment_pb2.Address(),
        ),
    )

def _build_void_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceVoidRequest(
        merchant_void_id="probe_void_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
    )
async def process_capture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Capture"""
    payment_client = PaymentClient(config)

    capture_response = await payment_client.capture(_build_capture_request("probe_connector_txn_001"))

    return {"status": capture_response.status}


async def process_create_client_authentication_token(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: MerchantAuthenticationService.CreateClientAuthenticationToken"""
    merchantauthentication_client = MerchantAuthenticationClient(config)

    create_response = await merchantauthentication_client.create_client_authentication_token(_build_create_client_authentication_token_request())

    return {"session_data": create_response.session_data}


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


async def process_tokenize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentMethodService.Tokenize"""
    paymentmethod_client = PaymentMethodClient(config)

    tokenize_response = await paymentmethod_client.tokenize(_build_tokenize_request())

    return {"token": tokenize_response.payment_method_token}


async def process_void(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Void"""
    payment_client = PaymentClient(config)

    void_response = await payment_client.void(_build_void_request("probe_connector_txn_001"))

    return {"status": void_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "capture"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
