# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py redsys
#
# Redsys — all integration scenarios and flows in one file.
# Run a scenario:  python3 redsys.py checkout_card

import asyncio
import sys
from payments import PaymentMethodAuthenticationClient
from payments import PaymentClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["authenticate", "capture", "get", "pre_authenticate", "refund", "refund_get", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        redsys=payment_pb2.RedsysConfig(
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            terminal_id=payment_methods_pb2.SecretString(value="YOUR_TERMINAL_ID"),
            sha256_pwd=payment_methods_pb2.SecretString(value="YOUR_SHA256_PWD"),
            base_url="YOUR_BASE_URL",
        ),
    ),
)




def _build_authenticate_request():
    return payment_pb2.PaymentMethodAuthenticationServiceAuthenticateRequest(
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        payment_method=payment_methods_pb2.PaymentMethod(  # Payment Method.
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
        authentication_data=payment_pb2.AuthenticationData(  # Authentication Details.
            eci="05",  # Electronic Commerce Indicator (ECI) from 3DS.
            cavv="AAAAAAAAAA==",  # Cardholder Authentication Verification Value (CAVV).
            threeds_server_transaction_id="probe-3ds-txn-001",  # 3DS Server Transaction ID.
            message_version="2.1.0",  # 3DS Message Version (e.g., "2.1.0", "2.2.0").
            ds_transaction_id="probe-ds-txn-001",  # Directory Server Transaction ID (DS Trans ID).
        ),
        return_url="https://example.com/3ds-return",  # URLs for Redirection.
        continue_redirection_url="https://example.com/3ds-continue",
        browser_info=payment_pb2.BrowserInformation(  # Contextual Information.
            color_depth=24,  # Display Information.
            screen_height=900,
            screen_width=1440,
            java_enabled=False,  # Browser Settings.
            java_script_enabled=True,
            language="en-US",
            time_zone_offset_minutes=-480,
            accept_header="application/json",  # Browser Headers.
            user_agent="Mozilla/5.0 (probe-bot)",
            accept_language="en-US,en;q=0.9",
            ip_address="1.2.3.4",  # Device Information.
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

def _build_get_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceGetRequest(
        merchant_transaction_id="probe_merchant_txn_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
    )

def _build_pre_authenticate_request():
    return payment_pb2.PaymentMethodAuthenticationServicePreAuthenticateRequest(
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        payment_method=payment_methods_pb2.PaymentMethod(  # Payment Method.
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
        enrolled_for_3ds=False,  # Authentication Details.
        return_url="https://example.com/3ds-return",  # URLs for Redirection.
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
    )

def _build_void_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceVoidRequest(
        merchant_void_id="probe_void_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
    )
async def process_authenticate(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentMethodAuthenticationService.Authenticate"""
    paymentmethodauthentication_client = PaymentMethodAuthenticationClient(config)

    authenticate_response = await paymentmethodauthentication_client.authenticate(_build_authenticate_request())

    return {"status": authenticate_response.status}


async def process_capture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Capture"""
    payment_client = PaymentClient(config)

    capture_response = await payment_client.capture(_build_capture_request("probe_connector_txn_001"))

    return {"status": capture_response.status}


async def process_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Get"""
    payment_client = PaymentClient(config)

    get_response = await payment_client.get(_build_get_request("probe_connector_txn_001"))

    return {"status": get_response.status}


async def process_pre_authenticate(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentMethodAuthenticationService.PreAuthenticate"""
    paymentmethodauthentication_client = PaymentMethodAuthenticationClient(config)

    pre_response = await paymentmethodauthentication_client.pre_authenticate(_build_pre_authenticate_request())

    return {"status": pre_response.status}


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
    scenario = sys.argv[1] if len(sys.argv) > 1 else "authenticate"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
