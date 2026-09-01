#!/usr/bin/env python3
"""Mock connector endpoint: answers any method/path with a Stripe-ish PaymentIntent JSON."""
import json
from http.server import BaseHTTPRequestHandler, HTTPServer

BODY = json.dumps({
    "id": "pi_mock_smoke_test",
    "object": "payment_intent",
    "status": "succeeded",
    "amount": 1000,
    "currency": "usd",
    "latest_charge": "ch_mock_1",
    "client_secret": "pi_mock_secret",
    # Required by the Stripe transformer's PaymentIntentResponse (a missing
    # `metadata` fails deserialization with RESPONSE_DESERIALIZATION_FAILED,
    # which made every local authorize record as grpc_status 13 and kept the
    # SUCCESS path of the transformer unexercised by the local rig).
    "metadata": {},
}).encode()

class Handler(BaseHTTPRequestHandler):
    def _respond(self):
        length = int(self.headers.get("content-length", 0) or 0)
        if length:
            self.rfile.read(length)
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(BODY)))
        self.end_headers()
        self.wfile.write(BODY)

    do_GET = do_POST = do_PUT = do_DELETE = do_PATCH = _respond

    def log_message(self, fmt, *args):
        print("mock-connector:", fmt % args, flush=True)

HTTPServer(("127.0.0.1", 3000), Handler).serve_forever()
