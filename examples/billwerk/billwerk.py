# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py billwerk
#
# Billwerk — all integration scenarios and flows in one file.
# Run a scenario:  python3 billwerk.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["capture", "get", "recurring_charge", "refund", "refund_get", "token_authorize", "token_setup_recurring", "tokenize", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     billwerk=payment_pb2.BillwerkConfig(api_key=...),
    # ),
)


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
        connector_order_reference_id="probe_order_ref_001",
    ))

    return {"status": get_response.status}


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


async def process_refund(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.refund"""
    payment_client = PaymentClient(config)

    # Step 1: Refund — return funds to the customer
    refund_response = await payment_client.refund(payment_pb2.TODO_FIX_MISSING_TYPE_refund(
        merchant_refund_id="probe_refund_001",
        connector_transaction_id="probe_connector_txn_001",
        payment_amount=1000,
        minor_amount=1000,
        currency="USD",
        reason="customer_request",
    ))

    if refund_response.status == "FAILED":
        raise RuntimeError(f"Refund failed: {refund_response.error}")

    return {"status": refund_response.status}


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


async def process_token_setup_recurring(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.token_setup_recurring"""
    payment_client = PaymentClient(config)

    # Step 1: token_setup_recurring
    token_response = await payment_client.token_setup_recurring(payment_pb2.TODO_FIX_MISSING_TYPE_token_setup_recurring(
        merchant_recurring_payment_id="probe_tokenized_mandate_001",
        minor_amount=0,
        currency="USD",
        connector_token="pm_1AbcXyzStripeTestToken",
        acceptance_type="ONLINE",
        accepted_at=0,
        ip_address="127.0.0.1",
        user_agent="Mozilla/5.0",
        amount=0,
        currency="USD",
        setup_future_usage="OFF_SESSION",
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
    scenario = sys.argv[1] if len(sys.argv) > 1 else "capture"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
