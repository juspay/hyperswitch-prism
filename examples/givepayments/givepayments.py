# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py givepayments
#
# Givepayments — all integration scenarios and flows in one file.
# Run a scenario:  python3 givepayments.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import RecurringPaymentClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["get", "recurring_charge", "refund", "refund_get"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        givepayments=payment_pb2.GivepaymentsConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            base_url="YOUR_BASE_URL",
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

def _build_recurring_charge_request():
    return payment_pb2.RecurringPaymentServiceChargeRequest(
        connector_recurring_payment_id=payment_pb2.MandateReference(  # Reference to existing mandate.
            connector_mandate_id=payment_pb2.ConnectorMandateReferenceId(  # mandate_id sent by the connector.
                connector_mandate_id="probe-mandate-123",
            ),
        ),
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        payment_method=payment_methods_pb2.PaymentMethod(  # Optional payment Method Information (for network transaction flows).
            token=payment_methods_pb2.TokenPaymentMethodType(
                token=payment_methods_pb2.SecretString(value="probe_pm_token"),  # The token string representing a payment method.
            ),
        ),
        return_url="https://example.com/recurring-return",
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=payment_pb2.Address(
                email=payment_methods_pb2.SecretString(value="test@example.com"),  # Contact Information.
            ),
        ),
        connector_customer_id="cust_probe_123",
        browser_info=payment_pb2.BrowserInformation(  # Browser Information.
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
        payment_method_type=payment_pb2.PaymentMethodType.Value("PAY_PAL"),
        off_session=True,  # Behavioral Flags and Preferences.
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
        refund_id="probe_refund_id_001",  # Deprecated.
    )
async def process_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Get"""
    payment_client = PaymentClient(config)

    get_response = await payment_client.get(_build_get_request("probe_connector_txn_001"))

    return {"status": get_response.status}


async def process_recurring_charge(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RecurringPaymentService.Charge"""
    recurringpayment_client = RecurringPaymentClient(config)

    recurring_response = await recurringpayment_client.charge(_build_recurring_charge_request())

    return {"status": recurring_response.status}


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

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "get"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
