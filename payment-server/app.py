"""
Payment Server - Flask application with PayPal and Cybersource integration.

Routing logic:
  USD -> PayPal
  EUR -> Cybersource

Endpoints:
  POST /authorize  - Authorize a payment
  POST /refund     - Refund a payment
  GET  /health     - Health check
"""

import asyncio
import os
from flask import Flask, request, jsonify

from config import load_credentials, build_paypal_config, build_cybersource_config
from payment_service import authorize_payment, refund_payment

app = Flask(__name__)

CREDS_PATH = os.environ.get("CREDS_PATH", "/home/grace/creds.json")

CREDS = load_credentials(CREDS_PATH)
CONFIGS = {
    "paypal": build_paypal_config(CREDS),
    "cybersource": build_cybersource_config(CREDS),
}


def run_async(coro):
    """Run an async coroutine from sync Flask context."""
    loop = asyncio.new_event_loop()
    try:
        return loop.run_until_complete(coro)
    finally:
        loop.close()


@app.route("/health", methods=["GET"])
def health():
    return jsonify({"status": "ok", "connectors": ["paypal", "cybersource"]})


@app.route("/authorize", methods=["POST"])
def authorize():
    """Authorize a payment.

    Request JSON:
      {
        "amount_minor": 1000,        # Amount in minor units (e.g. 1000 = $10.00)
        "currency": "USD",           # USD -> PayPal, EUR -> Cybersource
        "capture_method": "AUTOMATIC" # Optional, default AUTOMATIC
      }
    """
    data = request.get_json()
    if not data:
        return jsonify({"error": "Request body required"}), 400

    amount_minor = data.get("amount_minor")
    currency = data.get("currency")
    capture_method = data.get("capture_method", "AUTOMATIC")

    if not amount_minor or not currency:
        return jsonify({"error": "amount_minor and currency are required"}), 400

    try:
        result = run_async(
            authorize_payment(amount_minor, currency, CONFIGS, CREDS, capture_method)
        )
        return jsonify(result)
    except ValueError as e:
        return jsonify({"error": str(e)}), 400
    except RuntimeError as e:
        return jsonify({"error": str(e)}), 502
    except Exception as e:
        return jsonify({"error": f"{type(e).__name__}: {str(e)}"}), 500


@app.route("/refund", methods=["POST"])
def refund():
    """Refund a payment.

    Request JSON:
      {
        "connector_transaction_id": "...",  # From authorize response
        "amount_minor": 1000,               # Refund amount in minor units
        "currency": "USD",                  # Must match original payment currency
        "connector_feature_data": "..."     # Optional, from authorize response
      }
    """
    data = request.get_json()
    if not data:
        return jsonify({"error": "Request body required"}), 400

    connector_transaction_id = data.get("connector_transaction_id")
    amount_minor = data.get("amount_minor")
    currency = data.get("currency")
    connector_feature_data = data.get("connector_feature_data")

    if not all([connector_transaction_id, amount_minor, currency]):
        return jsonify(
            {"error": "connector_transaction_id, amount_minor, and currency are required"}
        ), 400

    try:
        result = run_async(
            refund_payment(
                connector_transaction_id, amount_minor, currency, CONFIGS, CREDS,
                connector_feature_data,
            )
        )
        return jsonify(result)
    except ValueError as e:
        return jsonify({"error": str(e)}), 400
    except RuntimeError as e:
        return jsonify({"error": str(e)}), 502
    except Exception as e:
        return jsonify({"error": f"{type(e).__name__}: {str(e)}"}), 500


if __name__ == "__main__":
    port = int(os.environ.get("PORT", 8080))
    print(f"Starting payment server on port {port}")
    print(f"Routing: USD -> PayPal, EUR -> Cybersource")
    app.run(host="0.0.0.0", port=port, debug=True)
