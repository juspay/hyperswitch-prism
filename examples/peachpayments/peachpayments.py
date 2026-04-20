# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py peachpayments
#
# Peachpayments — all integration scenarios and flows in one file.
# Run a scenario:  python3 peachpayments.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["authorize", "capture", "get", "parse_event", "proxy_authorize", "refund", "refund_get", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     peachpayments=payment_pb2.PeachpaymentsConfig(api_key=...),
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
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
    ))

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
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
    ))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Capture — settle the reserved funds
    capture_response = await payment_client.capture(payment_pb2.TODO_FIX_MISSING_TYPE_capture(
        merchant_capture_id="probe_capture_001",
        connector_transaction_id=authorize_response.connector_transaction_id,  # from Authorize response
        minor_amount=1000,
        currency="USD",
    ))

    if capture_response.status == "FAILED":
        raise RuntimeError(f"Capture failed: {capture_response.error}")

    return {"status": getattr(capture_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(capture_response, "error", None)}


async def process_refund(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Refund

    Return funds to the customer for a completed payment.
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
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
    ))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Refund — return funds to the customer
    refund_response = await payment_client.refund(payment_pb2.TODO_FIX_MISSING_TYPE_refund(
        merchant_refund_id="probe_refund_001",
        connector_transaction_id=authorize_response.connector_transaction_id,  # from Authorize response
        payment_amount=1000,
        minor_amount=1000,
        currency="USD",
        reason="customer_request",
    ))

    if refund_response.status == "FAILED":
        raise RuntimeError(f"Refund failed: {refund_response.error}")

    return {"status": getattr(refund_response, "status", ""), "error": getattr(refund_response, "error", None)}


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
        minor_amount=1000,
        currency="USD",
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
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
    ))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}


async def process_capture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.capture"""
    payment_client = PaymentClient(config)

    # Step 1: Capture — settle the reserved funds
    capture_response = await payment_client.capture(payment_pb2.TODO_FIX_MISSING_TYPE_capture(
        merchant_capture_id="probe_capture_001",
        connector_transaction_id="probe_connector_txn_001",
        minor_amount=1000,
        currency="USD",
    ))

    if capture_response.status == "FAILED":
        raise RuntimeError(f"Capture failed: {capture_response.error}")

    return {"status": capture_response.status}


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


async def process_parse_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.parse_event"""
    payment_client = PaymentClient(config)

    # Step 1: parse_event
    parse_response = await payment_client.parse_event(payment_pb2.TODO_FIX_MISSING_TYPE_parse_event(
        method="HTTP_METHOD_POST",
        uri="https://example.com/webhook",
        body="{\"webhookId\":\"probe_wh_001\",\"webhookType\":\"PAYMENT\",\"transaction\":{\"transactionId\":\"probe_txn_001\",\"referenceId\":\"probe_ref_001\",\"transactionResult\":\"ACK\",\"transactionType\":\"DEBIT\",\"paymentMethod\":\"probe_pm\"}}",
    ))

    return {"status": parse_response.status}


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
        minor_amount=1000,
        currency="USD",
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
