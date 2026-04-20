# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py stripe
#
# Stripe — all integration scenarios and flows in one file.
# Run a scenario:  python3 stripe.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["authorize", "capture", "create_client_authentication_token", "create_customer", "get", "incremental_authorization", "proxy_authorize", "proxy_setup_recurring", "recurring_charge", "refund", "refund_get", "setup_recurring", "token_authorize", "tokenize", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     stripe=payment_pb2.StripeConfig(api_key=...),
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


async def process_create_client_authentication_token(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.create_client_authentication_token"""
    payment_client = PaymentClient(config)

    # Step 1: create_client_authentication_token
    create_response = await payment_client.create_client_authentication_token(payment_pb2.TODO_FIX_MISSING_TYPE_create_client_authentication_token(
        merchant_client_session_id="probe_sdk_session_001",
        minor_amount=1000,
        currency="USD",
    ))

    return {"session_data": create_response.session_data}


async def process_create_customer(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.create_customer"""
    payment_client = PaymentClient(config)

    # Step 1: Create Customer — register customer record in the connector
    create_response = await payment_client.create(payment_pb2.TODO_FIX_MISSING_TYPE_create_customer(
        merchant_customer_id="cust_probe_123",
        customer_name="John Doe",
        email="test@example.com",
        phone_number="4155552671",
    ))

    return {"customer_id": create_response.connector_customer_id}


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


async def process_incremental_authorization(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.incremental_authorization"""
    payment_client = PaymentClient(config)

    # Step 1: incremental_authorization
    incremental_response = await payment_client.incremental_authorization(payment_pb2.TODO_FIX_MISSING_TYPE_incremental_authorization(
        merchant_authorization_id="probe_auth_001",
        connector_transaction_id="probe_connector_txn_001",
        minor_amount=1100,
        currency="USD",
        reason="incremental_auth_probe",
    ))

    return {"status": incremental_response.status}


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


async def process_proxy_setup_recurring(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.proxy_setup_recurring"""
    payment_client = PaymentClient(config)

    # Step 1: proxy_setup_recurring
    proxy_response = await payment_client.proxy_setup_recurring(payment_pb2.TODO_FIX_MISSING_TYPE_proxy_setup_recurring(
        merchant_recurring_payment_id="probe_proxy_mandate_001",
        minor_amount=0,
        currency="USD",
        card_number="4111111111111111",
        card_exp_month="03",
        card_exp_year="2030",
        card_cvc="123",
        card_holder_name="John Doe",
        acceptance_type="OFFLINE",
        accepted_at=0,
        auth_type="NO_THREE_DS",
        setup_future_usage="OFF_SESSION",
    ))

    return {"status": proxy_response.status}


async def process_recurring_charge(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.recurring_charge"""
    payment_client = PaymentClient(config)

    # Step 1: Recurring Charge — charge against the stored mandate
    recurring_response = await payment_client.charge(payment_pb2.TODO_FIX_MISSING_TYPE_recurring_charge(
        connector_mandate_id="probe-mandate-123",
        minor_amount=1000,
        currency="USD",
        token="probe_pm_token",
        return_url="https://example.com/recurring-return",
        connector_customer_id="cust_probe_123",
        payment_method_type="PAY_PAL",
        off_session=True,
    ))

    if recurring_response.status == "FAILED":
        raise RuntimeError(f"Recurring_Charge failed: {recurring_response.error}")

    return {"status": recurring_response.status}


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


async def process_setup_recurring(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.setup_recurring"""
    payment_client = PaymentClient(config)

    # Step 1: Setup Recurring — store the payment mandate
    setup_response = await payment_client.setup_recurring(payment_pb2.TODO_FIX_MISSING_TYPE_setup_recurring(
        merchant_recurring_payment_id="probe_mandate_001",
        minor_amount=0,
        currency="USD",
        card_number="4111111111111111",
        card_exp_month="03",
        card_exp_year="2030",
        card_cvc="737",
        card_holder_name="John Doe",
        auth_type="NO_THREE_DS",
        enrolled_for_3ds=False,
        return_url="https://example.com/mandate-return",
        setup_future_usage="OFF_SESSION",
        request_incremental_authorization=False,
        acceptance_type="OFFLINE",
        accepted_at=0,
    ))

    if setup_response.status == "FAILED":
        raise RuntimeError(f"Recurring setup failed: {setup_response.error}")
    if setup_response.status == "PENDING":
        # Mandate stored asynchronously — save connector_recurring_payment_id
        return {"status": "pending", "mandate_id": setup_response.connector_recurring_payment_id}

    return {"status": setup_response.status, "mandate_id": setup_response.connector_recurring_payment_id}


async def process_token_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.token_authorize"""
    payment_client = PaymentClient(config)

    # Step 1: token_authorize
    token_response = await payment_client.token_authorize(payment_pb2.TODO_FIX_MISSING_TYPE_token_authorize(
        merchant_transaction_id="probe_tokenized_txn_001",
        minor_amount=1000,
        currency="USD",
        connector_token="pm_1AbcXyzStripeTestToken",
        capture_method="AUTOMATIC",
        return_url="https://example.com/return",
    ))

    return {"status": token_response.status}


async def process_tokenize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.tokenize"""
    payment_client = PaymentClient(config)

    # Step 1: Tokenize — store card details and return a reusable token
    tokenize_response = await payment_client.tokenize(payment_pb2.TODO_FIX_MISSING_TYPE_tokenize(
        minor_amount=1000,
        currency="USD",
        card_number="4111111111111111",
        card_exp_month="03",
        card_exp_year="2030",
        card_cvc="737",
        card_holder_name="John Doe",
    ))

    return {"token": tokenize_response.payment_method_token}


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
