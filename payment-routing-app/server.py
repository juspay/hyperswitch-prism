#!/usr/bin/env python3
"""
Payment Routing Server

Routes payments to connectors based on currency:
  - USD -> Authorizedotnet
  - EUR -> Cybersource

Supports authorization and refund flows for both connectors.
"""

import asyncio
import json
import sys
import uuid
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse, parse_qs

from payments import PaymentClient, SecretString
from payments.generated import sdk_config_pb2, payment_pb2
from google.protobuf.json_format import ParseDict


CREDS_PATH = "/home/grace/creds.json"

CURRENCY_CONNECTOR_MAP = {
    "USD": "authorizedotnet",
    "EUR": "cybersource",
}


def load_credentials(path: str) -> dict:
    with open(path) as f:
        return json.load(f)


def build_authorizedotnet_config(creds: dict) -> sdk_config_pb2.ConnectorConfig:
    account = creds["authorizedotnet"]["connector_account_details"]
    cfg = sdk_config_pb2.ConnectorConfig(
        options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    )
    cfg.connector_config.CopyFrom(
        payment_pb2.ConnectorSpecificConfig(
            authorizedotnet=payment_pb2.AuthorizedotnetConfig(
                name=SecretString(value=account["api_key"]),
                transaction_key=SecretString(value=account["key1"]),
            )
        )
    )
    return cfg


def build_cybersource_config(creds: dict) -> sdk_config_pb2.ConnectorConfig:
    account = creds["cybersource"]["connector_1"]["connector_account_details"]
    cfg = sdk_config_pb2.ConnectorConfig(
        options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    )
    cfg.connector_config.CopyFrom(
        payment_pb2.ConnectorSpecificConfig(
            cybersource=payment_pb2.CybersourceConfig(
                api_key=SecretString(value=account["api_key"]),
                merchant_account=SecretString(value=account["key1"]),
                api_secret=SecretString(value=account["api_secret"]),
            )
        )
    )
    return cfg


def get_connector_config(currency: str, configs: dict) -> tuple:
    """Return (connector_name, config) based on currency routing rules."""
    connector = CURRENCY_CONNECTOR_MAP.get(currency)
    if not connector:
        raise ValueError(
            f"Unsupported currency: {currency}. Supported: {list(CURRENCY_CONNECTOR_MAP.keys())}"
        )
    return connector, configs[connector]


def build_authorize_request(
    amount_minor: int,
    currency: str,
    card_number: str = "4111111111111111",
    card_exp_month: str = "03",
    card_exp_year: str = "2030",
    card_cvc: str = "737",
    card_holder_name: str = "John Doe",
    capture_method: str = "AUTOMATIC",
    merchant_txn_id: str = None,
    email: str = "test@example.com",
) -> payment_pb2.PaymentServiceAuthorizeRequest:
    if not merchant_txn_id:
        merchant_txn_id = f"txn_{uuid.uuid4().hex[:12]}"

    data = {
        "merchant_transaction_id": merchant_txn_id,
        "amount": {
            "minor_amount": amount_minor,
            "currency": currency,
        },
        "payment_method": {
            "card": {
                "card_number": {"value": card_number},
                "card_exp_month": {"value": card_exp_month},
                "card_exp_year": {"value": card_exp_year},
                "card_cvc": {"value": card_cvc},
                "card_holder_name": {"value": card_holder_name},
            }
        },
        "capture_method": capture_method,
        "address": {
            "billing_address": {
                "first_name": {"value": card_holder_name.split()[0]},
                "last_name": {"value": card_holder_name.split()[-1]},
                "line1": {"value": "123 Main St"},
                "city": {"value": "San Francisco"},
                "state": {"value": "CA"},
                "zip_code": {"value": "94105"},
                "country_alpha2_code": "US",
                "email": {"value": email},
            }
        },
        "auth_type": "NO_THREE_DS",
        "return_url": "https://example.com/return",
    }

    # Cybersource requires customer email in a separate field too
    if currency == "EUR":
        data["customer"] = {"email": {"value": email}}

    return ParseDict(data, payment_pb2.PaymentServiceAuthorizeRequest())


def build_refund_request(
    connector_transaction_id: str,
    amount_minor: int,
    currency: str,
    merchant_refund_id: str = None,
    card_number: str = "4111111111111111",
    card_exp_month: str = "03",
    card_exp_year: str = "2030",
) -> payment_pb2.PaymentServiceRefundRequest:
    if not merchant_refund_id:
        merchant_refund_id = f"ref_{uuid.uuid4().hex[:12]}"

    # Authorizedotnet requires card details in connector_feature_data for refunds
    feature_data = json.dumps({
        "creditCard": {
            "cardNumber": card_number,
            "expirationDate": f"{card_exp_month}{card_exp_year[-2:]}",
        }
    })

    return ParseDict(
        {
            "merchant_refund_id": merchant_refund_id,
            "connector_transaction_id": connector_transaction_id,
            "payment_amount": amount_minor,
            "refund_amount": {
                "minor_amount": amount_minor,
                "currency": currency,
            },
            "reason": "customer_request",
            "connector_feature_data": {"value": feature_data},
        },
        payment_pb2.PaymentServiceRefundRequest(),
    )


async def authorize_payment(
    configs: dict,
    amount_minor: int,
    currency: str,
    card_number: str = "4111111111111111",
    card_exp_month: str = "03",
    card_exp_year: str = "2030",
    card_cvc: str = "737",
    card_holder_name: str = "John Doe",
    capture_method: str = "AUTOMATIC",
) -> dict:
    """Authorize a payment, routing to the correct connector based on currency."""
    connector_name, config = get_connector_config(currency, configs)
    client = PaymentClient(config)

    request = build_authorize_request(
        amount_minor=amount_minor,
        currency=currency,
        card_number=card_number,
        card_exp_month=card_exp_month,
        card_exp_year=card_exp_year,
        card_cvc=card_cvc,
        card_holder_name=card_holder_name,
        capture_method=capture_method,
    )

    response = await client.authorize(request)
    status_name = payment_pb2.PaymentStatus.Name(response.status)

    return {
        "connector": connector_name,
        "currency": currency,
        "amount_minor": amount_minor,
        "status": status_name,
        "connector_transaction_id": response.connector_transaction_id,
        "error": str(response.error) if response.status == payment_pb2.FAILURE else None,
    }


async def refund_payment(
    configs: dict,
    connector_transaction_id: str,
    amount_minor: int,
    currency: str,
) -> dict:
    """Refund a previously authorized payment."""
    connector_name, config = get_connector_config(currency, configs)
    client = PaymentClient(config)

    request = build_refund_request(
        connector_transaction_id=connector_transaction_id,
        amount_minor=amount_minor,
        currency=currency,
    )

    response = await client.refund(request)

    # The SDK may return PaymentStatus values for failed refunds
    try:
        status_name = payment_pb2.RefundStatus.Name(response.status)
    except ValueError:
        try:
            status_name = payment_pb2.PaymentStatus.Name(response.status)
        except ValueError:
            status_name = f"UNKNOWN_{response.status}"

    is_failure = response.status not in (
        payment_pb2.REFUND_SUCCESS,
        payment_pb2.REFUND_PENDING,
        payment_pb2.REFUND_MANUAL_REVIEW,
    )

    return {
        "connector": connector_name,
        "currency": currency,
        "amount_minor": amount_minor,
        "status": status_name,
        "refund_id": getattr(response, "connector_refund_id", ""),
        "error": str(response.error) if is_failure else None,
    }


class PaymentHandler(BaseHTTPRequestHandler):
    """HTTP request handler for payment operations."""

    configs = None

    def _read_body(self) -> dict:
        length = int(self.headers.get("Content-Length", 0))
        if length == 0:
            return {}
        return json.loads(self.rfile.read(length))

    def _respond(self, status: int, data: dict):
        body = json.dumps(data, indent=2, default=str).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        path = urlparse(self.path).path

        if path == "/authorize":
            self._handle_authorize()
        elif path == "/refund":
            self._handle_refund()
        else:
            self._respond(404, {"error": f"Unknown endpoint: {path}"})

    def do_GET(self):
        path = urlparse(self.path).path
        if path == "/health":
            self._respond(200, {"status": "ok", "routing": CURRENCY_CONNECTOR_MAP})
        else:
            self._respond(404, {"error": f"Unknown endpoint: {path}"})

    def _handle_authorize(self):
        try:
            body = self._read_body()
            currency = body.get("currency", "USD")
            amount_minor = body.get("amount_minor", 1000)
            capture_method = body.get("capture_method", "AUTOMATIC")

            result = asyncio.run(
                authorize_payment(
                    configs=self.configs,
                    amount_minor=amount_minor,
                    currency=currency,
                    card_number=body.get("card_number", "4111111111111111"),
                    card_exp_month=body.get("card_exp_month", "03"),
                    card_exp_year=body.get("card_exp_year", "2030"),
                    card_cvc=body.get("card_cvc", "737"),
                    card_holder_name=body.get("card_holder_name", "John Doe"),
                    capture_method=capture_method,
                )
            )
            status_code = 200 if result["status"] not in ("FAILURE", "AUTHORIZATION_FAILED") else 400
            self._respond(status_code, result)
        except ValueError as e:
            self._respond(400, {"error": str(e)})
        except Exception as e:
            self._respond(500, {"error": str(e), "type": type(e).__name__})

    def _handle_refund(self):
        try:
            body = self._read_body()
            connector_txn_id = body.get("connector_transaction_id")
            if not connector_txn_id:
                self._respond(400, {"error": "connector_transaction_id is required"})
                return

            currency = body.get("currency", "USD")
            amount_minor = body.get("amount_minor", 1000)

            result = asyncio.run(
                refund_payment(
                    configs=self.configs,
                    connector_transaction_id=connector_txn_id,
                    amount_minor=amount_minor,
                    currency=currency,
                )
            )
            status_code = 200 if result["status"] != "REFUND_FAILURE" else 400
            self._respond(status_code, result)
        except ValueError as e:
            self._respond(400, {"error": str(e)})
        except Exception as e:
            self._respond(500, {"error": str(e), "type": type(e).__name__})

    def log_message(self, format, *args):
        print(f"[{self.log_date_time_string()}] {format % args}")


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080

    creds = load_credentials(CREDS_PATH)
    configs = {
        "authorizedotnet": build_authorizedotnet_config(creds),
        "cybersource": build_cybersource_config(creds),
    }
    PaymentHandler.configs = configs

    server = HTTPServer(("0.0.0.0", port), PaymentHandler)
    print(f"Payment Routing Server running on port {port}")
    print(f"Routing: {json.dumps(CURRENCY_CONNECTOR_MAP, indent=2)}")
    print(f"Endpoints:")
    print(f"  POST /authorize  - Authorize a payment")
    print(f"  POST /refund     - Refund a payment")
    print(f"  GET  /health     - Health check")

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down...")
        server.server_close()


if __name__ == "__main__":
    main()
