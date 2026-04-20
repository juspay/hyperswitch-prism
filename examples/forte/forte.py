# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py forte
#
# Forte — all integration scenarios and flows in one file.
# Run a scenario:  python3 forte.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["authorize", "get", "proxy_authorize", "refund_get", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     forte=payment_pb2.ForteConfig(api_key=...),
    # ),
)


async def process_checkout_autocapture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """One-step Payment (Authorize + Capture)

    Simple payment that authorizes and captures in one call. Use for immediate charges.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(payment_pb2.TODO_FIX_MISSING_TYPE_authorize(
        merchant_transaction_id="probe_txn_001",
        minor_amount=1000,
        currency="USD",
        card_number="4111111111111111",
        card_exp_month="03",
        card_exp_year="2030",
        card_cvc="737",
        card_holder_name="John Doe",
        capture_method="AUTOMATIC",
        first_name="John",
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
    ))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    return {"status": getattr(authorize_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(authorize_response, "error", None)}


async def process_void_payment(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Void Payment

    Cancel an authorized but not-yet-captured payment.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(payment_pb2.TODO_FIX_MISSING_TYPE_authorize(
        merchant_transaction_id="probe_txn_001",
        minor_amount=1000,
        currency="USD",
        card_number="4111111111111111",
        card_exp_month="03",
        card_exp_year="2030",
        card_cvc="737",
        card_holder_name="John Doe",
        capture_method="MANUAL",
        first_name="John",
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
    ))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Void — release reserved funds (cancel authorization)
    void_response = await payment_client.void(payment_pb2.TODO_FIX_MISSING_TYPE_void(
        merchant_void_id="probe_void_001",
        connector_transaction_id=authorize_response.connector_transaction_id,  # from Authorize response
    ))

    return {"status": getattr(void_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(void_response, "error", None)}


async def process_get_payment(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Get Payment Status

    Retrieve current payment status from the connector.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(payment_pb2.TODO_FIX_MISSING_TYPE_authorize(
        merchant_transaction_id="probe_txn_001",
        minor_amount=1000,
        currency="USD",
        card_number="4111111111111111",
        card_exp_month="03",
        card_exp_year="2030",
        card_cvc="737",
        card_holder_name="John Doe",
        capture_method="MANUAL",
        first_name="John",
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
    ))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Get — retrieve current payment status from the connector
    get_response = await payment_client.get(payment_pb2.TODO_FIX_MISSING_TYPE_get(
        merchant_transaction_id="probe_merchant_txn_001",
        connector_transaction_id=authorize_response.connector_transaction_id,  # from Authorize response
        minor_amount=1000,
        currency="USD",
    ))

    return {"status": getattr(get_response, "status", ""), "transaction_id": getattr(get_response, "connector_transaction_id", ""), "error": getattr(get_response, "error", None)}


async def process_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.authorize (Card)"""
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(payment_pb2.TODO_FIX_MISSING_TYPE_authorize(
        merchant_transaction_id="probe_txn_001",
        minor_amount=1000,
        currency="USD",
        card_number="4111111111111111",
        card_exp_month="03",
        card_exp_year="2030",
        card_cvc="737",
        card_holder_name="John Doe",
        capture_method="AUTOMATIC",
        first_name="John",
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
    ))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}


async def process_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.get"""
    payment_client = PaymentClient(config)

    # Step 1: Get — retrieve current payment status from the connector
    get_response = await payment_client.get(payment_pb2.TODO_FIX_MISSING_TYPE_get(
        merchant_transaction_id="probe_merchant_txn_001",
        connector_transaction_id="probe_connector_txn_001",
        minor_amount=1000,
        currency="USD",
    ))

    return {"status": get_response.status}


async def process_proxy_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.proxy_authorize"""
    payment_client = PaymentClient(config)

    # Step 1: proxy_authorize
    proxy_response = await payment_client.proxy_authorize(payment_pb2.TODO_FIX_MISSING_TYPE_proxy_authorize(
        merchant_transaction_id="probe_proxy_txn_001",
        minor_amount=1000,
        currency="USD",
        card_number="4111111111111111",
        card_exp_month="03",
        card_exp_year="2030",
        card_cvc="123",
        card_holder_name="John Doe",
        first_name="John",
        capture_method="AUTOMATIC",
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
    ))

    return {"status": proxy_response.status}


async def process_refund_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.refund_get"""
    payment_client = PaymentClient(config)

    # Step 1: refund_get
    refund_response = await payment_client.refund_get(payment_pb2.TODO_FIX_MISSING_TYPE_refund_get(
        merchant_refund_id="probe_refund_001",
        connector_transaction_id="probe_connector_txn_001",
        refund_id="probe_refund_id_001",
    ))

    return {"status": refund_response.status}


async def process_void(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.void"""
    payment_client = PaymentClient(config)

    # Step 1: Void — release reserved funds (cancel authorization)
    void_response = await payment_client.void(payment_pb2.TODO_FIX_MISSING_TYPE_void(
        merchant_void_id="probe_void_001",
        connector_transaction_id="probe_connector_txn_001",
    ))

    return {"status": void_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "checkout_autocapture"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
