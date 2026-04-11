"""
Payment Server Application
Routes USD payments to Shift4 and EUR payments to Fiuu.
Supports authorization and refund flows via hyperswitch-prism SDK.
"""

import asyncio
import json
import uuid
from flask import Flask, request, jsonify
from google.protobuf.json_format import ParseDict
from payments import PaymentClient, SecretString
from payments.generated import sdk_config_pb2, payment_pb2

app = Flask(__name__)

# ---------------------------------------------------------------------------
# Credentials & connector configs
# ---------------------------------------------------------------------------

CREDS_PATH = "/home/grace/creds.json"

def _load_creds():
    with open(CREDS_PATH) as f:
        return json.load(f)

_creds = _load_creds()

def _build_shift4_config():
    """Shift4 uses header-key auth: api_key only."""
    cfg = sdk_config_pb2.ConnectorConfig(
        options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    )
    cfg.connector_config.CopyFrom(
        payment_pb2.ConnectorSpecificConfig(
            shift4=payment_pb2.Shift4Config(
                api_key=SecretString(value=_creds["shift4"]["connector_account_details"]["api_key"]),
            )
        )
    )
    return cfg

def _build_fiuu_config():
    """Fiuu uses signature-key auth: api_key->verify_key, key1->merchant_id, api_secret->secret_key."""
    fiuu_creds = _creds["fiuu"]["connector_account_details"]
    cfg = sdk_config_pb2.ConnectorConfig(
        options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    )
    cfg.connector_config.CopyFrom(
        payment_pb2.ConnectorSpecificConfig(
            fiuu=payment_pb2.FiuuConfig(
                merchant_id=SecretString(value=fiuu_creds["key1"]),
                verify_key=SecretString(value=fiuu_creds["api_key"]),
                secret_key=SecretString(value=fiuu_creds["api_secret"]),
            )
        )
    )
    return cfg

SHIFT4_CONFIG = _build_shift4_config()
FIUU_CONFIG = _build_fiuu_config()

# ---------------------------------------------------------------------------
# Routing logic: USD -> Shift4, EUR -> Fiuu
# ---------------------------------------------------------------------------

CURRENCY_CONNECTOR_MAP = {
    "USD": ("shift4", SHIFT4_CONFIG),
    "EUR": ("fiuu", FIUU_CONFIG),
}

def _get_connector_for_currency(currency: str):
    currency = currency.upper()
    if currency not in CURRENCY_CONNECTOR_MAP:
        return None, None
    return CURRENCY_CONNECTOR_MAP[currency]

# ---------------------------------------------------------------------------
# Helper: run async in sync Flask context
# ---------------------------------------------------------------------------

def _run_async(coro):
    loop = asyncio.new_event_loop()
    try:
        return loop.run_until_complete(coro)
    finally:
        loop.close()

# ---------------------------------------------------------------------------
# Payment status helpers
# ---------------------------------------------------------------------------

def _payment_status_name(status_int):
    try:
        return payment_pb2.PaymentStatus.Name(status_int)
    except ValueError:
        return str(status_int)

def _refund_status_name(status_int):
    try:
        return payment_pb2.RefundStatus.Name(status_int)
    except ValueError:
        return str(status_int)

# ---------------------------------------------------------------------------
# API Endpoints
# ---------------------------------------------------------------------------

@app.route("/health", methods=["GET"])
def health():
    return jsonify({"status": "ok"})


@app.route("/authorize", methods=["POST"])
def authorize():
    """
    Authorize a payment. Routes based on currency.

    Request JSON:
    {
        "amount": 1000,           // minor units (e.g. 1000 = $10.00)
        "currency": "USD",        // "USD" -> Shift4, "EUR" -> Fiuu
        "card_number": "4111111111111111",
        "card_exp_month": "03",
        "card_exp_year": "2030",
        "card_cvc": "737",
        "card_holder_name": "John Doe",
        "capture_method": "AUTOMATIC"  // optional, defaults to AUTOMATIC
    }
    """
    data = request.get_json()
    if not data:
        return jsonify({"error": "Request body required"}), 400

    currency = data.get("currency", "USD").upper()
    connector_name, config = _get_connector_for_currency(currency)
    if not config:
        return jsonify({"error": f"Unsupported currency: {currency}. Supported: USD (Shift4), EUR (Fiuu)"}), 400

    txn_id = data.get("transaction_id", f"txn_{uuid.uuid4().hex[:12]}")
    capture_method = data.get("capture_method", "AUTOMATIC")

    authorize_request_dict = {
        "merchant_transaction_id": txn_id,
        "amount": {
            "minor_amount": data.get("amount", 1000),
            "currency": currency,
        },
        "payment_method": {
            "card": {
                "card_number": {"value": data.get("card_number", "4111111111111111")},
                "card_exp_month": {"value": data.get("card_exp_month", "03")},
                "card_exp_year": {"value": data.get("card_exp_year", "2030")},
                "card_cvc": {"value": data.get("card_cvc", "737")},
                "card_holder_name": {"value": data.get("card_holder_name", "John Doe")},
            }
        },
        "capture_method": capture_method,
        "address": {
            "billing_address": {
                "first_name": {"value": data.get("card_holder_name", "John Doe").split()[0]},
            }
        },
        "auth_type": "NO_THREE_DS",
        "return_url": "https://example.com/return",
    }

    # Fiuu requires webhook_url and customer email
    if connector_name == "fiuu":
        authorize_request_dict["webhook_url"] = "https://example.com/webhook"
        authorize_request_dict["customer"] = {
            "email": {"value": data.get("email", "customer@example.com")},
        }

    auth_req = ParseDict(authorize_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())

    try:
        client = PaymentClient(config)
        resp = _run_async(client.authorize(auth_req))

        return jsonify({
            "connector": connector_name,
            "currency": currency,
            "transaction_id": txn_id,
            "connector_transaction_id": resp.connector_transaction_id,
            "status": _payment_status_name(resp.status),
            "status_code": resp.status,
        })
    except Exception as e:
        return jsonify({
            "error": str(e),
            "error_type": type(e).__name__,
            "error_details": {k: str(v) for k, v in vars(e).items()} if hasattr(e, '__dict__') else {},
            "connector": connector_name,
            "currency": currency,
        }), 502


@app.route("/refund", methods=["POST"])
def refund():
    """
    Refund a previously authorized payment.

    Request JSON:
    {
        "connector_transaction_id": "...",  // from authorize response
        "amount": 1000,                     // refund amount in minor units
        "currency": "USD",                  // must match original currency
        "reason": "customer_request"        // optional
    }
    """
    data = request.get_json()
    if not data:
        return jsonify({"error": "Request body required"}), 400

    currency = data.get("currency", "USD").upper()
    connector_name, config = _get_connector_for_currency(currency)
    if not config:
        return jsonify({"error": f"Unsupported currency: {currency}. Supported: USD (Shift4), EUR (Fiuu)"}), 400

    connector_txn_id = data.get("connector_transaction_id")
    if not connector_txn_id:
        return jsonify({"error": "connector_transaction_id is required"}), 400

    refund_amount = data.get("amount", 1000)
    refund_id = data.get("refund_id", f"ref_{uuid.uuid4().hex[:12]}")

    refund_request_dict = {
        "merchant_refund_id": refund_id,
        "connector_transaction_id": connector_txn_id,
        "payment_amount": refund_amount,
        "refund_amount": {
            "minor_amount": refund_amount,
            "currency": currency,
        },
        "reason": data.get("reason", "customer_request"),
    }

    # Fiuu requires webhook_url for refunds
    if connector_name == "fiuu":
        refund_request_dict["webhook_url"] = "https://example.com/webhook"

    refund_req = ParseDict(refund_request_dict, payment_pb2.PaymentServiceRefundRequest())

    try:
        client = PaymentClient(config)
        resp = _run_async(client.refund(refund_req))

        return jsonify({
            "connector": connector_name,
            "currency": currency,
            "refund_id": refund_id,
            "connector_transaction_id": connector_txn_id,
            "status": _refund_status_name(resp.status),
            "status_code": resp.status,
        })
    except Exception as e:
        return jsonify({
            "error": str(e),
            "connector": connector_name,
            "currency": currency,
        }), 502


@app.route("/authorize-and-refund", methods=["POST"])
def authorize_and_refund():
    """
    End-to-end flow: authorize a payment then immediately refund it.
    Useful for testing both flows in a single call.

    Request JSON:
    {
        "amount": 1000,
        "currency": "USD",
        "card_number": "4111111111111111",
        "card_exp_month": "03",
        "card_exp_year": "2030",
        "card_cvc": "737",
        "card_holder_name": "John Doe"
    }
    """
    data = request.get_json()
    if not data:
        return jsonify({"error": "Request body required"}), 400

    currency = data.get("currency", "USD").upper()
    connector_name, config = _get_connector_for_currency(currency)
    if not config:
        return jsonify({"error": f"Unsupported currency: {currency}. Supported: USD (Shift4), EUR (Fiuu)"}), 400

    amount = data.get("amount", 1000)
    txn_id = f"txn_{uuid.uuid4().hex[:12]}"

    # Step 1: Authorize
    authorize_request_dict = {
        "merchant_transaction_id": txn_id,
        "amount": {
            "minor_amount": amount,
            "currency": currency,
        },
        "payment_method": {
            "card": {
                "card_number": {"value": data.get("card_number", "4111111111111111")},
                "card_exp_month": {"value": data.get("card_exp_month", "03")},
                "card_exp_year": {"value": data.get("card_exp_year", "2030")},
                "card_cvc": {"value": data.get("card_cvc", "737")},
                "card_holder_name": {"value": data.get("card_holder_name", "John Doe")},
            }
        },
        "capture_method": "AUTOMATIC",
        "address": {
            "billing_address": {
                "first_name": {"value": data.get("card_holder_name", "John Doe").split()[0]},
            }
        },
        "auth_type": "NO_THREE_DS",
        "return_url": "https://example.com/return",
    }

    if connector_name == "fiuu":
        authorize_request_dict["webhook_url"] = "https://example.com/webhook"
        authorize_request_dict["customer"] = {
            "email": {"value": data.get("email", "customer@example.com")},
        }

    auth_req = ParseDict(authorize_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())

    async def _authorize_and_refund_flow():
        client = PaymentClient(config)
        auth_resp = await client.authorize(auth_req)
        auth_status = _payment_status_name(auth_resp.status)

        result = {
            "connector": connector_name,
            "currency": currency,
            "authorize": {
                "transaction_id": txn_id,
                "connector_transaction_id": auth_resp.connector_transaction_id,
                "status": auth_status,
                "status_code": auth_resp.status,
            },
        }

        # Step 2: Refund (only if authorization succeeded)
        if auth_resp.connector_transaction_id:
            refund_id = f"ref_{uuid.uuid4().hex[:12]}"
            refund_request_dict = {
                "merchant_refund_id": refund_id,
                "connector_transaction_id": auth_resp.connector_transaction_id,
                "payment_amount": amount,
                "refund_amount": {
                    "minor_amount": amount,
                    "currency": currency,
                },
                "reason": "customer_request",
            }

            if connector_name == "fiuu":
                refund_request_dict["webhook_url"] = "https://example.com/webhook"

            refund_req = ParseDict(refund_request_dict, payment_pb2.PaymentServiceRefundRequest())
            refund_resp = await client.refund(refund_req)

            result["refund"] = {
                "refund_id": refund_id,
                "status": _refund_status_name(refund_resp.status),
                "status_code": refund_resp.status,
            }
        else:
            result["refund"] = {"error": "Skipped - no connector_transaction_id from authorize"}

        return result

    try:
        result = _run_async(_authorize_and_refund_flow())
        return jsonify(result)

    except Exception as e:
        return jsonify({
            "error": str(e),
            "connector": connector_name,
            "currency": currency,
        }), 502


if __name__ == "__main__":
    print("Payment Server starting...")
    print("Routing: USD -> Shift4, EUR -> Fiuu")
    print("Endpoints: /authorize, /refund, /authorize-and-refund, /health")
    app.run(host="0.0.0.0", port=8080, debug=True)
