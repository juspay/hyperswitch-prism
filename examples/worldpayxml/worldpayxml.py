# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py worldpayxml
#
# Worldpayxml — all integration scenarios and flows in one file.
# Run a scenario:  python3 worldpayxml.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Standalone credentials (field names depend on connector auth type):
# _default_config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     worldpayxml=payment_pb2.WorldpayxmlConfig(api_key=...),
# ))




def _build_authorize_request(capture_method: str):
    return payment_pb2.PaymentServiceAuthorizeRequest(
        merchant_transaction_id="probe_txn_001",  # Identification.
        amount=payment_pb2.Money(  # The amount for the payment.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        payment_method=payment_methods_pb2.PaymentMethod(  # Payment method to be used.
            card=payment_methods_pb2.CardDetails(
                card_number=payment_methods_pb2.CardNumberType(value="4111111111111111"),  # Card Identification.
                card_exp_month=payment_methods_pb2.SecretString(value="03"),
                card_exp_year=payment_methods_pb2.SecretString(value="2030"),
                card_cvc=payment_methods_pb2.SecretString(value="737"),
                card_holder_name=payment_methods_pb2.SecretString(value="John Doe"),  # Cardholder Information.
            ),
        ),
        capture_method=payment_pb2.CaptureMethod.Value(capture_method),  # Method for capturing the payment.
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=payment_pb2.Address(),
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),  # Authentication Details.
        return_url="https://example.com/return",  # URLs for Redirection and Webhooks.
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

def _build_proxy_authorize_request():
    return payment_pb2.PaymentServiceProxyAuthorizeRequest(
        merchant_transaction_id="probe_proxy_txn_001",
        amount=payment_pb2.Money(
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        card_proxy=payment_methods_pb2.CardDetails(  # Card proxy for vault-aliased payments (VGS, Basis Theory, Spreedly). Real card values are substituted by the proxy before reaching the connector.
            card_number=payment_methods_pb2.CardNumberType(value="4111111111111111"),  # Card Identification.
            card_exp_month=payment_methods_pb2.SecretString(value="03"),
            card_exp_year=payment_methods_pb2.SecretString(value="2030"),
            card_cvc=payment_methods_pb2.SecretString(value="123"),
            card_holder_name=payment_methods_pb2.SecretString(value="John Doe"),  # Cardholder Information.
        ),
        address=payment_pb2.PaymentAddress(
            billing_address=payment_pb2.Address(),
        ),
        capture_method=payment_pb2.CaptureMethod.Value("AUTOMATIC"),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),
        return_url="https://example.com/return",
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
    )
async def process_checkout_autocapture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """One-step Payment (Authorize + Capture)

    Simple payment that authorizes and captures in one call. Use for immediate charges.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    return {"status": getattr(authorize_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(authorize_response, "error", None)}


async def process_checkout_card(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Card Payment (Authorize + Capture)

    Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("MANUAL"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Capture — settle the reserved funds
    capture_response = await payment_client.capture(_build_capture_request(authorize_response.connector_transaction_id))

    if capture_response.status == "FAILED":
        raise RuntimeError(f"Capture failed: {capture_response.error}")

    return {"status": getattr(capture_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(capture_response, "error", None)}


async def process_refund(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Refund

    Return funds to the customer for a completed payment.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Refund — return funds to the customer
    refund_response = await payment_client.refund(_build_refund_request(authorize_response.connector_transaction_id))

    if refund_response.status == "FAILED":
        raise RuntimeError(f"Refund failed: {refund_response.error}")

    return {"status": getattr(refund_response, "status", ""), "error": getattr(refund_response, "error", None)}


async def process_void_payment(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Void Payment

    Cancel an authorized but not-yet-captured payment.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("MANUAL"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Void — release reserved funds (cancel authorization)
    void_response = await payment_client.void(_build_void_request(authorize_response.connector_transaction_id))

    return {"status": getattr(void_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(void_response, "error", None)}


async def process_get_payment(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Get Payment Status

    Retrieve current payment status from the connector.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("MANUAL"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Get — retrieve current payment status from the connector
    get_response = await payment_client.get(_build_get_request(authorize_response.connector_transaction_id))

    return {"status": getattr(get_response, "status", ""), "transaction_id": getattr(get_response, "connector_transaction_id", ""), "error": getattr(get_response, "error", None)}


async def process_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Authorize (Card)"""
    payment_client = PaymentClient(config)

    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}


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


async def process_proxy_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.ProxyAuthorize"""
    payment_client = PaymentClient(config)

    proxy_response = await payment_client.proxy_authorize(_build_proxy_authorize_request())

    return {"status": proxy_response.status}


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
    scenario = sys.argv[1] if len(sys.argv) > 1 else "checkout_autocapture"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
