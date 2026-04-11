# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py cashfree
#
# Cashfree — all integration scenarios and flows in one file.
# Run a scenario:  python3 cashfree.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Standalone credentials (field names depend on connector auth type):
# _default_config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     cashfree=payment_pb2.CashfreeConfig(api_key=...),
# ))




def _build_authorize_request(capture_method: str):
    return payment_pb2.PaymentServiceAuthorizeRequest(
        merchant_transaction_id="probe_txn_001",  # Identification.
        amount=payment_pb2.Money(  # The amount for the payment.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        payment_method=payment_methods_pb2.PaymentMethod(  # Payment method to be used.
            upi_collect=payment_methods_pb2.UpiCollect(
                vpa_id=payment_methods_pb2.SecretString(value="test@upi"),  # Virtual Payment Address.
            ),
        ),
        capture_method=payment_pb2.CaptureMethod.Value(capture_method),  # Method for capturing the payment.
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=payment_pb2.Address(),
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),  # Authentication Details.
        return_url="https://example.com/return",  # URLs for Redirection and Webhooks.
        connector_order_id="connector_order_id",  # Send the connector order identifier here if an order was created before authorize.
    )
async def process_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Authorize (UpiCollect)"""
    payment_client = PaymentClient(config)

    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "authorize"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
