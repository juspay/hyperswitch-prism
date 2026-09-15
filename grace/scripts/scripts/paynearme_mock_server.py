#!/usr/bin/env python3
"""Local mock of the PayNearMe API v3.0 JSON interface.

Implements the five endpoints the UCS `paynearme` connector talks to, with
response bodies copied from the shapes in the technical specification
(including the documented inconsistencies: `orders` vs `order`, bare-number vs
quoted-string identifiers/amounts, and the `cancelled` spelling).

Every request is logged verbatim (method, path, headers, body) and the HMAC
signature is recomputed locally so the log records whether the signature UCS
computed matches the spec algorithm.

Usage:  python3 paynearme_mock_server.py [port] [secret_key]
"""

import hashlib
import hmac
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 9099
SECRET_KEY = sys.argv[2] if len(sys.argv) > 2 else "MOCK_PNM_API_SECRET_KEY"

EXEMPT = {"format", "signature", "call"}

PNM_ORDER_ID = "85237034088"
PNM_PAYMENT_ID = "798183461108"
PAYMENT_METHOD_ID = "a95ac4a03ef38"


def expected_signature(body: dict) -> str:
    pairs = []
    for key, value in body.items():
        if key in EXEMPT:
            continue
        if value is None:
            continue
        if isinstance(value, bool):
            value = "true" if value else "false"
        pairs.append((key, str(value)))
    pairs.sort(key=lambda kv: kv[0])
    string_to_sign = "".join(f"{k}{v}" for k, v in pairs)
    return (
        hmac.new(SECRET_KEY.encode(), string_to_sign.encode(), hashlib.sha256).hexdigest(),
        string_to_sign,
    )


# ---------------------------------------------------------------------------
# Spec-shaped responses
# ---------------------------------------------------------------------------

def create_order_response(_body):
    # /create_order answers with the PLURAL key `orders` and a BARE-NUMBER id.
    return 201, {
        "status": "ok",
        "orders": {
            "site_name": "Mock PayNearMe Site",
            "site_identifier": _body.get("site_identifier", "S2411573363"),
            "pnm_order_crid": "cyu9Qg",
            "pnm_order_identifier": int(PNM_ORDER_ID),
            "pnm_order_short_identifier": "LSN19S",
            "site_order_key": 567060000,
            "order_created": "2026-09-02 21:36:15 -0700",
            "order_status": "open",
            "order_amount": _body.get("order_amount", "500.00"),
            "order_currency": "USD",
            "order_type": _body.get("order_type", "exact"),
            "order_is_standing": False,
            "electronic_payments": {
                "customer": {
                    "pnm_customer_identifier": "U5506959413",
                    "site_customer_identifier": _body.get("site_customer_identifier"),
                }
            },
        },
    }


def create_payment_method_response(body):
    # /create_payment_method answers with the SINGULAR key `order`, the created
    # payment nested at `order.payments[]`, and a BARE-NUMBER payment id.
    return 201, {
        "response_code": "0",
        "status": "ok",
        "order": {
            "site_name": "Mock PayNearMe Site",
            "site_order_identifier": "470070000",
            "type": "order",
            "site_identifier": body.get("site_identifier"),
            "pnm_order_identifier": body.get("pnm_order_identifier", PNM_ORDER_ID),
            "order_status": "open",
            "order_amount": body.get("payment_amount", "500.00"),
            "order_currency": "USD",
            "electronic_payments": {
                "payment_methods": [
                    {
                        "type": "debit",
                        "fee_amount": "4.99",
                        "fee_currency": "USD",
                        "accounts": [
                            {
                                "payment_method_identifier": PAYMENT_METHOD_ID,
                                "status": "active",
                                "name": body.get("payment_method_billing_name"),
                                "description": "Debit Card",
                                "account_type": "Debit",
                                "number": "6651",
                                "fee_amount": "4.99",
                                "fee_currency": "USD",
                                "expiration_date": body.get("payment_method_card_expiry_pii"),
                                "card_brand": "PULSE",
                            }
                        ],
                    }
                ]
            },
            "customer": {
                "pnm_customer_identifier": "U4509315551",
                "site_customer_identifier": "470070000",
            },
            "payments": [
                {
                    "payment_made": "2026-09-02 14:44:35 -0800",
                    "payment_amount": 100,
                    "payment_currency": "USD",
                    "payment_status": "approved",
                    "payment_type": "debit",
                    "payment_account": "Debit Card 6651",
                    "payment_method_identifier": PAYMENT_METHOD_ID,
                    "net_payment_amount": 100,
                    "net_payment_currency": "USD",
                    "payment_processing_fee": 0,
                    "settled_to_site": False,
                    "pnm_payment_identifier": int(PNM_PAYMENT_ID),
                    "site_payment_identifier": body.get("site_payment_identifier"),
                    "pricing_schedule_name": "consumer",
                    "site_channel": "consumer",
                }
            ],
        },
    }


def find_payment_response(body):
    return 201, {
        "status": "ok",
        "payment": {
            "payment_made": "2026-09-02 13:41:31 -0700",
            "payment_amount": "504.99",
            "payment_currency": "USD",
            "payment_status": "approved",
            "payment_type": "debit",
            "payment_account": "Debit Card 6651",
            "payment_method_identifier": PAYMENT_METHOD_ID,
            "net_payment_amount": "500.00",
            "net_payment_currency": "USD",
            "payment_processing_fee": "4.99",
            "settled_to_site": "false",
            "pnm_payment_identifier": body.get("pnm_payment_identifier", PNM_PAYMENT_ID),
            "site_payment_identifier": "0123456789",
            "pricing_schedule_name": "consumer",
            "site_channel": "consumer",
        },
    }


def cancel_payment_response(body):
    # NOTE: the documented example spells the status `cancelled` (two l's) while
    # the OpenAPI enum declares `canceled`. The mock returns the doubled spelling
    # deliberately so the connector's alias is exercised.
    return 201, {
        "status": "ok",
        "payment": {
            "payment_made": "2026-09-02 14:02:05 -0700",
            "payment_amount": "560.00",
            "payment_currency": "USD",
            "payment_status": "cancelled",
            "payment_type": "debit",
            "payment_method_identifier": "405eb56f20202",
            "pnm_payment_identifier": body.get("pnm_payment_identifier", PNM_PAYMENT_ID),
            "net_payment_amount": "500.00",
            "settled_to_site": "false",
        },
    }


def refund_payment_response(body):
    return 201, {
        "status": "ok",
        "payment": {
            "payment_made": "2026-09-02 12:34:38 -0700",
            "payment_amount": "210.99",
            "payment_currency": "USD",
            "payment_status": "refunded",
            "payment_type": "credit",
            "payment_account": "Credit Card 1512",
            "payment_method_identifier": "198fd2a399aae",
            "net_payment_amount": "200.00",
            "pnm_payment_identifier": body.get("pnm_payment_identifier", PNM_PAYMENT_ID),
            "refund": {
                "refund_status": "started",
                "refund_amount": body.get("refund_amount", "210.99"),
                "refund_currency": "USD",
                "unsettle_to_merchant_amount": "200.00",
                "unsettle_to_merchant_currency": "USD",
                "unsettled_from_collector_amount": "0.00",
            },
        },
    }


ROUTES = {
    "/json-api/create_order": create_order_response,
    "/json-api/create_payment_method": create_payment_method_response,
    "/json-api/find_payment": find_payment_response,
    "/json-api/cancel_payment": cancel_payment_response,
    "/json-api/refund_payment": refund_payment_response,
}


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass  # replaced by the explicit dump below

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        raw = self.rfile.read(length) if length else b""
        try:
            body = json.loads(raw.decode() or "{}")
        except Exception:
            body = {}

        print("=" * 78, flush=True)
        print(f">>> INBOUND REQUEST  {self.command} {self.path} HTTP/{self.request_version}",
              flush=True)
        print("--- headers ---", flush=True)
        for name, value in self.headers.items():
            print(f"{name}: {value}", flush=True)
        print("--- body ---", flush=True)
        print(json.dumps(body, indent=2, sort_keys=True), flush=True)

        if isinstance(body, dict) and "signature" in body:
            expected, string_to_sign = expected_signature(body)
            got = body.get("signature")
            print("--- signature verification ---", flush=True)
            print(f"string_to_sign : {string_to_sign}", flush=True)
            print(f"expected       : {expected}", flush=True)
            print(f"received       : {got}", flush=True)
            print(f"MATCH          : {expected == got}", flush=True)

        handler = ROUTES.get(self.path)
        if handler is None:
            status, payload = 400, {
                "status": "error",
                "errors": [f"Unknown endpoint {self.path}"],
            }
        else:
            status, payload = handler(body if isinstance(body, dict) else {})

        encoded = json.dumps(payload).encode()
        print("--- response ---", flush=True)
        print(f"HTTP {status}", flush=True)
        print(json.dumps(payload, indent=2), flush=True)
        print("=" * 78, flush=True)

        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)


if __name__ == "__main__":
    print(f"PayNearMe mock listening on http://127.0.0.1:{PORT}/json-api", flush=True)
    HTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
