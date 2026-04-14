# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py revolut
#
# Revolut — all integration scenarios and flows in one file.
# Run a scenario:  python3 revolut.py checkout_card

import asyncio
import sys
from google.protobuf.json_format import ParseDict
from payments import PaymentClient
from payments import EventClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Standalone credentials (field names depend on connector auth type):
# _default_config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     revolut=payment_pb2.RevolutConfig(api_key=...),
# ))




def _build_authorize_request(capture_method: str):
    return ParseDict(
        {
            "merchant_transaction_id": "probe_txn_001",  # Identification.
            "amount": {  # The amount for the payment.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "payment_method": {  # Payment method to be used
                "card": {  # Generic card payment
                    "card_number": "4111111111111111",  # Card Identification
                    "card_exp_month": "03",
                    "card_exp_year": "2030",
                    "card_cvc": "737",
                    "card_holder_name": "John Doe"  # Cardholder Information
                }
            },
            "capture_method": capture_method,  # Method for capturing the payment.
            "address": {  # Address Information.
                "billing_address": {
                }
            },
            "auth_type": "NO_THREE_DS",  # Authentication Details.
            "return_url": "https://example.com/return"  # URLs for Redirection and Webhooks.
        },
        payment_pb2.PaymentServiceAuthorizeRequest(),
    )

def _build_capture_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_capture_id": "probe_capture_001",  # Identification.
            "connector_transaction_id": connector_transaction_id,
            "amount_to_capture": {  # Capture Details.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            }
        },
        payment_pb2.PaymentServiceCaptureRequest(),
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

def _build_handle_event_request():
    return ParseDict(
        {
        },
        payment_pb2.EventServiceHandleRequest(),
    )

def _build_proxy_authorize_request():
    return ParseDict(
        {
            "merchant_transaction_id": "probe_proxy_txn_001",
            "amount": {
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "card_proxy": {  # Card proxy for vault-aliased payments (VGS, Basis Theory, Spreedly). Real card values are substituted by the proxy before reaching the connector.
                "card_number": {"value": "4111111111111111"},  # Card Identification.
                "card_exp_month": {"value": "03"},
                "card_exp_year": {"value": "2030"},
                "card_cvc": {"value": "123"},
                "card_holder_name": {"value": "John Doe"}  # Cardholder Information.
            },
            "address": {
                "billing_address": {
                }
            },
            "capture_method": "AUTOMATIC",
            "auth_type": "NO_THREE_DS",
            "return_url": "https://example.com/return"
        },
        payment_pb2.PaymentServiceProxyAuthorizeRequest(),
    )

def _build_refund_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_refund_id": "probe_refund_001",  # Identification.
            "connector_transaction_id": connector_transaction_id,
            "payment_amount": 1000,  # Amount Information.
            "refund_amount": {
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "reason": "customer_request"  # Reason for the refund.
        },
        payment_pb2.PaymentServiceRefundRequest(),
    )

def _build_refund_get_request():
    return ParseDict(
        {
            "merchant_refund_id": "probe_refund_001",  # Identification.
            "connector_transaction_id": "probe_connector_txn_001",
            "refund_id": "probe_refund_id_001"
        },
        payment_pb2.RefundServiceGetRequest(),
    )

def _build_token_authorize_request():
    return ParseDict(
        {
            "merchant_transaction_id": "probe_tokenized_txn_001",
            "amount": {
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "connector_token": {"value": "pm_1AbcXyzStripeTestToken"},  # Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
            "address": {
                "billing_address": {
                }
            },
            "capture_method": "AUTOMATIC",
            "return_url": "https://example.com/return"
        },
        payment_pb2.PaymentServiceTokenAuthorizeRequest(),
    )

def _build_verify_redirect_request():
    return ParseDict(
        {
        },
        payment_pb2.PaymentServiceVerifyRedirectResponseRequest(),
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


async def process_checkout_autocapture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Card Payment (Automatic Capture)

    Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.
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


async def process_checkout_wallet(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Wallet Payment (Google Pay / Apple Pay)

    Wallet payments pass an encrypted token from the browser/device SDK. Pass the token blob directly — do not decrypt client-side.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(ParseDict(
        {
            "merchant_transaction_id": "probe_txn_001",  # Identification
            "amount": {  # The amount for the payment
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00)
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR")
            },
            "payment_method": {  # Payment method to be used
                "google_pay": {  # Google Pay
                    "type": "CARD",  # Type of payment method
                    "description": "Visa 1111",  # User-facing description of the payment method
                    "info": {
                        "card_network": "VISA",  # Card network name
                        "card_details": "1111"  # Card details (usually last 4 digits)
                    },
                    "tokenization_data": {
                        "encrypted_data": {  # Encrypted Google Pay payment data
                            "token_type": "PAYMENT_GATEWAY",  # The type of the token
                            "token": "{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}"  # Token generated for the wallet
                        }
                    }
                }
            },
            "capture_method": "AUTOMATIC",  # Method for capturing the payment
            "address": {  # Address Information
                "billing_address": {
                }
            },
            "auth_type": "NO_THREE_DS",  # Authentication Details
            "return_url": "https://example.com/return"  # URLs for Redirection and Webhooks
        },
        payment_pb2.PaymentServiceAuthorizeRequest(),
    ))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    return {"status": getattr(authorize_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(authorize_response, "error", None)}


async def process_checkout_bank(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Bank Transfer (SEPA / ACH / BACS)

    Direct bank debit (Sepa). Bank transfers typically use `capture_method=AUTOMATIC`.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(ParseDict(
        {
            "merchant_transaction_id": "probe_txn_001",  # Identification
            "amount": {  # The amount for the payment
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00)
                "currency": "EUR"  # ISO 4217 currency code (e.g., "USD", "EUR")
            },
            "payment_method": {  # Payment method to be used
                "sepa": {  # Sepa - Single Euro Payments Area direct debit
                    "iban": "DE89370400440532013000",  # International bank account number (iban) for SEPA
                    "bank_account_holder_name": "John Doe"  # Owner name for bank debit
                }
            },
            "capture_method": "AUTOMATIC",  # Method for capturing the payment
            "address": {  # Address Information
                "billing_address": {
                }
            },
            "auth_type": "NO_THREE_DS",  # Authentication Details
            "return_url": "https://example.com/return"  # URLs for Redirection and Webhooks
        },
        payment_pb2.PaymentServiceAuthorizeRequest(),
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


async def authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Authorize (Card)"""
    payment_client = PaymentClient(config)

    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}


async def capture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Capture"""
    payment_client = PaymentClient(config)

    capture_response = await payment_client.capture(_build_capture_request("probe_connector_txn_001"))

    return {"status": capture_response.status}


async def get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Get"""
    payment_client = PaymentClient(config)

    get_response = await payment_client.get(_build_get_request("probe_connector_txn_001"))

    return {"status": get_response.status}


async def handle_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: EventService.HandleEvent"""
    event_client = EventClient(config)

    handle_response = await event_client.handle_event(_build_handle_event_request())

    return {"status": handle_response.status}


async def proxy_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.ProxyAuthorize"""
    payment_client = PaymentClient(config)

    proxy_response = await payment_client.proxy_authorize(_build_proxy_authorize_request())

    return {"status": proxy_response.status}


async def refund(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Refund"""
    payment_client = PaymentClient(config)

    refund_response = await payment_client.refund(_build_refund_request("probe_connector_txn_001"))

    return {"status": refund_response.status}


async def refund_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RefundService.Get"""
    refund_client = RefundClient(config)

    refund_response = await refund_client.refund_get(_build_refund_get_request())

    return {"status": refund_response.status}


async def token_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.TokenAuthorize"""
    payment_client = PaymentClient(config)

    token_response = await payment_client.token_authorize(_build_token_authorize_request())

    return {"status": token_response.status}


async def verify_redirect(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.VerifyRedirectResponse"""
    payment_client = PaymentClient(config)

    verify_response = await payment_client.verify_redirect(_build_verify_redirect_request())

    return {"status": verify_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "checkout_autocapture"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
