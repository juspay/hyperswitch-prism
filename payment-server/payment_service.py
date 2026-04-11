"""
Payment service handling authorization and refund flows.

Supports PayPal (with access token management) and Cybersource.
"""

import uuid
import base64
import httpx
from google.protobuf.json_format import ParseDict

from payments import PaymentClient, IntegrationError, ConnectorError
from payments.generated import sdk_config_pb2, payment_pb2

from router import get_config_for_currency

PAYPAL_SANDBOX_TOKEN_URL = "https://api-m.sandbox.paypal.com/v1/oauth2/token"


async def _get_paypal_access_token(creds: dict) -> dict:
    """Obtain a PayPal OAuth2 access token via direct HTTP.

    The SDK FFI doesn't include a create_server_authentication_token transformer,
    so we call PayPal's OAuth2 endpoint directly.
    """
    paypal_creds = creds["paypal"]["connector_account_details"]
    client_id = paypal_creds["key1"]
    client_secret = paypal_creds["api_key"]

    auth_header = base64.b64encode(
        f"{client_id}:{client_secret}".encode()
    ).decode()

    async with httpx.AsyncClient() as client:
        response = await client.post(
            PAYPAL_SANDBOX_TOKEN_URL,
            headers={
                "Authorization": f"Basic {auth_header}",
                "Content-Type": "application/x-www-form-urlencoded",
            },
            data="grant_type=client_credentials",
        )
        response.raise_for_status()
        token_data = response.json()

    return {
        "access_token": {
            "token": {"value": token_data["access_token"]},
            "expires_in_seconds": token_data.get("expires_in", 3600),
            "token_type": token_data.get("token_type", "Bearer"),
        }
    }


def _build_authorize_request(
    amount_minor: int,
    currency: str,
    capture_method: str,
    connector_name: str,
    state: dict = None,
) -> payment_pb2.PaymentServiceAuthorizeRequest:
    """Build an authorization request for any connector."""
    txn_id = f"txn_{uuid.uuid4().hex[:12]}"

    payload = {
        "merchant_transaction_id": txn_id,
        "amount": {
            "minor_amount": amount_minor,
            "currency": currency.upper(),
        },
        "payment_method": {
            "card": {
                "card_number": {"value": "4111111111111111"},
                "card_exp_month": {"value": "03"},
                "card_exp_year": {"value": "2030"},
                "card_cvc": {"value": "737"},
                "card_holder_name": {"value": "John Doe"},
            }
        },
        "capture_method": capture_method,
        "address": {"billing_address": {}},
        "auth_type": "NO_THREE_DS",
        "return_url": "https://example.com/return",
    }

    if connector_name == "cybersource":
        payload["customer"] = {"email": {"value": "test@example.com"}}
        payload["address"] = {
            "billing_address": {
                "first_name": {"value": "John"},
                "last_name": {"value": "Doe"},
                "line1": {"value": "123 Main St"},
                "city": {"value": "San Francisco"},
                "state": {"value": "CA"},
                "zip_code": {"value": "94105"},
                "country_alpha2_code": "US",
            }
        }

    if connector_name == "paypal" and state:
        payload["state"] = state

    return ParseDict(payload, payment_pb2.PaymentServiceAuthorizeRequest())


def _build_refund_request(
    connector_transaction_id: str,
    amount_minor: int,
    currency: str,
    connector_name: str,
    state: dict = None,
    connector_feature_data: str = None,
) -> payment_pb2.PaymentServiceRefundRequest:
    """Build a refund request for any connector."""
    refund_id = f"refund_{uuid.uuid4().hex[:12]}"

    payload = {
        "merchant_refund_id": refund_id,
        "connector_transaction_id": connector_transaction_id,
        "payment_amount": amount_minor,
        "refund_amount": {
            "minor_amount": amount_minor,
            "currency": currency.upper(),
        },
        "reason": "customer_request",
    }

    if connector_name == "paypal" and state:
        payload["state"] = state

    if connector_feature_data:
        payload["connector_feature_data"] = {"value": connector_feature_data}

    return ParseDict(payload, payment_pb2.PaymentServiceRefundRequest())


def _extract_error(response) -> str | None:
    """Safely extract error message from a response."""
    try:
        if response.HasField("error"):
            err = response.error
            parts = []
            if err.code:
                parts.append(f"code={err.code}")
            if err.message:
                parts.append(err.message)
            if err.reason:
                parts.append(f"reason={err.reason}")
            return "; ".join(parts) if parts else None
    except (ValueError, AttributeError):
        pass
    return None


async def authorize_payment(
    amount_minor: int,
    currency: str,
    configs: dict[str, sdk_config_pb2.ConnectorConfig],
    creds: dict,
    capture_method: str = "AUTOMATIC",
) -> dict:
    """Authorize a payment, routing by currency.

    Returns dict with connector, status, transaction_id, and any error.
    """
    connector_name, config = get_config_for_currency(currency, configs)

    state = None
    if connector_name == "paypal":
        state = await _get_paypal_access_token(creds)

    request = _build_authorize_request(
        amount_minor, currency, capture_method, connector_name, state
    )

    client = PaymentClient(config)
    response = await client.authorize(request)

    result = {
        "connector": connector_name,
        "currency": currency.upper(),
        "amount_minor": amount_minor,
        "status": response.status,
        "connector_transaction_id": response.connector_transaction_id,
    }

    # Preserve connector_feature_data for use in subsequent flows (e.g. refund)
    try:
        if response.HasField("connector_feature_data"):
            result["connector_feature_data"] = response.connector_feature_data.value
    except (ValueError, AttributeError):
        pass

    error = _extract_error(response)
    if error:
        result["error"] = error

    return result


async def refund_payment(
    connector_transaction_id: str,
    amount_minor: int,
    currency: str,
    configs: dict[str, sdk_config_pb2.ConnectorConfig],
    creds: dict,
    connector_feature_data: str = None,
) -> dict:
    """Refund a payment, routing by currency.

    Returns dict with connector, status, and any error.
    """
    connector_name, config = get_config_for_currency(currency, configs)

    state = None
    if connector_name == "paypal":
        state = await _get_paypal_access_token(creds)

    request = _build_refund_request(
        connector_transaction_id, amount_minor, currency, connector_name, state,
        connector_feature_data,
    )

    client = PaymentClient(config)
    response = await client.refund(request)

    result = {
        "connector": connector_name,
        "currency": currency.upper(),
        "amount_minor": amount_minor,
        "status": response.status,
    }

    error = _extract_error(response)
    if error:
        result["error"] = error

    return result
