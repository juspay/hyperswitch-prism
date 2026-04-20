# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py trustpay
#
# Trustpay — all integration scenarios and flows in one file.
# Run a scenario:  python3 trustpay.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["authorize", "create_order", "create_server_authentication_token", "get", "parse_event", "proxy_authorize", "recurring_charge", "refund", "refund_get"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     trustpay=payment_pb2.TrustpayConfig(api_key=...),
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
        email="test@example.com",
        first_name="John",
        line1="123 Main St",
        city="Seattle",
        zip_code="98101",
        country_alpha2_code="US",
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
        user_agent="Mozilla/5.0 (probe-bot)",
        ip_address="1.2.3.4",
        token="probe_access_token",
        expires_in_seconds=3600,
        token_type="Bearer",
    ))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    return {"status": getattr(authorize_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(authorize_response, "error", None)}


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
        email="test@example.com",
        first_name="John",
        line1="123 Main St",
        city="Seattle",
        zip_code="98101",
        country_alpha2_code="US",
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
        user_agent="Mozilla/5.0 (probe-bot)",
        ip_address="1.2.3.4",
        token="probe_access_token",
        expires_in_seconds=3600,
        token_type="Bearer",
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
        token="probe_access_token",
        expires_in_seconds=3600,
        token_type="Bearer",
    ))

    if refund_response.status == "FAILED":
        raise RuntimeError(f"Refund failed: {refund_response.error}")

    return {"status": getattr(refund_response, "status", ""), "error": getattr(refund_response, "error", None)}


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
        email="test@example.com",
        first_name="John",
        line1="123 Main St",
        city="Seattle",
        zip_code="98101",
        country_alpha2_code="US",
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
        user_agent="Mozilla/5.0 (probe-bot)",
        ip_address="1.2.3.4",
        token="probe_access_token",
        expires_in_seconds=3600,
        token_type="Bearer",
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
        token="probe_access_token",
        expires_in_seconds=3600,
        token_type="Bearer",
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
        email="test@example.com",
        first_name="John",
        line1="123 Main St",
        city="Seattle",
        zip_code="98101",
        country_alpha2_code="US",
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
        user_agent="Mozilla/5.0 (probe-bot)",
        ip_address="1.2.3.4",
        token="probe_access_token",
        expires_in_seconds=3600,
        token_type="Bearer",
    ))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}


async def process_create_order(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.create_order"""
    payment_client = PaymentClient(config)

    # Step 1: create_order
    create_response = await payment_client.create_order(payment_pb2.TODO_FIX_MISSING_TYPE_create_order(
        merchant_order_id="probe_order_001",
        minor_amount=1000,
        currency="USD",
        token="probe_access_token",
        expires_in_seconds=3600,
        token_type="Bearer",
    ))

    return {"status": create_response.status}


async def process_create_server_authentication_token(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.create_server_authentication_token"""
    payment_client = PaymentClient(config)

    # Step 1: create_server_authentication_token
    create_response = await payment_client.create_server_authentication_token(payment_pb2.TODO_FIX_MISSING_TYPE_create_server_authentication_token(
        # No required fields
    ))

    return {"status": create_response.status}


async def process_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.get"""
    payment_client = PaymentClient(config)

    # Step 1: Get — retrieve current payment status from the connector
    get_response = await payment_client.get(payment_pb2.TODO_FIX_MISSING_TYPE_get(
        merchant_transaction_id="probe_merchant_txn_001",
        connector_transaction_id="probe_connector_txn_001",
        minor_amount=1000,
        currency="USD",
        token="probe_access_token",
        expires_in_seconds=3600,
        token_type="Bearer",
    ))

    return {"status": get_response.status}


async def process_parse_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.parse_event"""
    payment_client = PaymentClient(config)

    # Step 1: parse_event
    parse_response = await payment_client.parse_event(payment_pb2.TODO_FIX_MISSING_TYPE_parse_event(
        method="HTTP_METHOD_POST",
        uri="https://example.com/webhook",
        body="{\"PaymentInformation\":{\"CreditDebitIndicator\":\"CRDT\",\"References\":{\"EndToEndId\":\"probe_txn_001\"},\"Status\":\"Paid\",\"Amount\":{\"InstructedAmount\":10.00,\"Currency\":\"EUR\"}},\"Signature\":\"probe_sig\"}",
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
        email="test@example.com",
        first_name="John",
        line1="123 Main St",
        city="Seattle",
        zip_code="98101",
        country_alpha2_code="US",
        capture_method="AUTOMATIC",
        auth_type="NO_THREE_DS",
        return_url="https://example.com/return",
        user_agent="Mozilla/5.0 (probe-bot)",
        ip_address="1.2.3.4",
        token="probe_access_token",
        expires_in_seconds=3600,
        token_type="Bearer",
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
        token="probe_access_token",
        expires_in_seconds=3600,
        token_type="Bearer",
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
        token="probe_access_token",
        expires_in_seconds=3600,
        token_type="Bearer",
    ))

    return {"status": refund_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "checkout_autocapture"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
