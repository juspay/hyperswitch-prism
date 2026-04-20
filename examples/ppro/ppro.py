# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py ppro
#
# Ppro — all integration scenarios and flows in one file.
# Run a scenario:  python3 ppro.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["capture", "get", "parse_event", "recurring_charge", "refund", "refund_get", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     ppro=payment_pb2.PproConfig(api_key=...),
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
    ))

    return {"status": get_response.status}


async def process_parse_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.parse_event"""
    payment_client = PaymentClient(config)

    # Step 1: parse_event
    parse_response = await payment_client.parse_event(payment_pb2.TODO_FIX_MISSING_TYPE_parse_event(
        method="HTTP_METHOD_POST",
        uri="https://example.com/webhook",
        body="{\"specversion\":\"1.0\",\"type\":\"PAYMENT_CHARGE_SUCCESS\",\"source\":\"probe_source\",\"id\":\"probe_event_001\",\"time\":\"2024-01-01T00:00:00Z\",\"data\":{\"charge\":{\"id\":\"probe_txn_001\",\"status\":\"SUCCEEDED\",\"amount\":1000,\"currency\":\"EUR\"}}}",
    ))

    return {"status": parse_response.status}


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
    scenario = sys.argv[1] if len(sys.argv) > 1 else "capture"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
