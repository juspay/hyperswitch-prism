# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py payu
#
# Payu — all integration scenarios and flows in one file.
# Run a scenario:  python3 payu.py checkout_card

import asyncio
import sys
from google.protobuf.json_format import ParseDict
from payments import PaymentClient
from payments import RecurringPaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Standalone credentials (field names depend on connector auth type):
# _default_config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     payu=payment_pb2.PayuConfig(api_key=...),
# ))




def _build_authorize_request(capture_method: str):
    return ParseDict(
        {
            "merchant_transaction_id": "probe_txn_001",  # Identification.
            "amount": {  # The amount for the payment.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "payment_method": {  # Payment method to be used.
                "upi_collect": {  # UPI Collect.
                    "vpa_id": {"value": "test@upi"}  # Virtual Payment Address.
                }
            },
            "capture_method": capture_method,  # Method for capturing the payment.
            "address": {  # Address Information.
                "billing_address": {
                    "first_name": {"value": "John"},  # Personal Information.
                    "email": {"value": "test@example.com"},  # Contact Information.
                    "phone_number": {"value": "4155552671"},
                    "phone_country_code": "+1"
                }
            },
            "auth_type": "NO_THREE_DS",  # Authentication Details.
            "return_url": "https://example.com/return",  # URLs for Redirection and Webhooks.
            "browser_info": {
                "ip_address": "1.2.3.4"  # Device Information.
            }
        },
        payment_pb2.PaymentServiceAuthorizeRequest(),
    )

def _build_get_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_transaction_id": "probe_merchant_txn_001",  # Identification.
            "connector_transaction_id": connector_transaction_id,
            "amount": {  # Amount Information.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            }
        },
        payment_pb2.PaymentServiceGetRequest(),
    )

def _build_recurring_charge_request():
    return ParseDict(
        {
            "connector_recurring_payment_id": {  # Reference to existing mandate.
                "connector_mandate_id": {  # mandate_id sent by the connector.
                    "connector_mandate_id": "probe-mandate-123"
                }
            },
            "amount": {  # Amount Information.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "payment_method": {  # Optional payment Method Information (for network transaction flows).
                "token": {  # Payment tokens.
                    "token": {"value": "probe_pm_token"}  # The token string representing a payment method.
                }
            },
            "return_url": "https://example.com/recurring-return",
            "address": {  # Address Information.
                "billing_address": {
                    "first_name": {"value": "John"},  # Personal Information.
                    "email": {"value": "test@example.com"},  # Contact Information.
                    "phone_number": {"value": "4155552671"},
                    "phone_country_code": "+1"
                }
            },
            "connector_customer_id": "cust_probe_123",
            "browser_info": {  # Browser Information.
                "ip_address": "1.2.3.4"  # Device Information.
            },
            "payment_method_type": "PAY_PAL",
            "off_session": True  # Behavioral Flags and Preferences.
        },
        payment_pb2.RecurringPaymentServiceChargeRequest(),
    )
async def authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Authorize (UpiCollect)"""
    payment_client = PaymentClient(config)

    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}


async def get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Get"""
    payment_client = PaymentClient(config)

    get_response = await payment_client.get(_build_get_request("probe_connector_txn_001"))

    return {"status": get_response.status}


async def recurring_charge(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RecurringPaymentService.Charge"""
    recurringpayment_client = RecurringPaymentClient(config)

    recurring_response = await recurringpayment_client.charge(_build_recurring_charge_request())

    return {"status": recurring_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "authorize"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
