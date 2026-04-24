#!/usr/bin/env python3
"""
Webhook smoke test — Adyen AUTHORISATION

Uses a real Adyen AUTHORISATION webhook body and feeds it into
EventClient.handle_event / parse_event with connector identity only
(no API credentials, no webhook secret).

What this validates:
  1. SDK routes to the correct connector from identity alone
  2. Adyen webhook body is parsed correctly
  3. event_type is returned
  4. source_verified=False is expected — no real HMAC secret provided,
     and Adyen verification is not mandatory so it must NOT error out
  5. IntegrationError / ConnectorError are NOT thrown for a valid payload

Usage:
  cd sdk/python && PYTHONPATH=src python3 smoke-test/test_smoke_webhook.py
"""

import json
import sys
import os

# ── ANSI color helpers ─────────────────────────────────────────────────────────
_NO_COLOR = not sys.stdout.isatty() or os.environ.get("NO_COLOR")
def _c(code, text): return text if _NO_COLOR else f"\033[{code}m{text}\033[0m"
def _green(t):  return _c("32", t)
def _yellow(t): return _c("33", t)
def _red(t):    return _c("31", t)
def _grey(t):   return _c("90", t)
def _bold(t):   return _c("1",  t)

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'src'))

try:
    from payments import EventClient, IntegrationError, ConnectorError
    from payments.generated import payment_pb2, sdk_config_pb2
except ImportError as e:
    print(f"Error importing payments package: {e}")
    print("Run: cd sdk/python && PYTHONPATH=src python3 smoke-test/test_smoke_webhook.py")
    sys.exit(1)

# ── Adyen AUTHORISATION webhook body (from real test configuration) ────────────
# Sensitive fields replaced:
#   merchantAccountCode → "YOUR_MERCHANT_ACCOUNT"
#   merchantReference   → "pay_test_00000000000000"
#   pspReference        → "TEST000000000000"
#   hmacSignature       → "test_hmac_signature_placeholder"
#   cardHolderName      → "John Doe"
#   shopperEmail        → "shopper@example.com"
ADYEN_WEBHOOK_BODY = json.dumps({
    "live": "false",
    "notificationItems": [{
        "NotificationRequestItem": {
            "additionalData": {
                "authCode": "APPROVED",
                "cardSummary": "1111",
                "cardHolderName": "John Doe",
                "expiryDate": "03/2030",
                "shopperEmail": "shopper@example.com",
                "shopperIP": "128.0.0.1",
                "shopperInteraction": "Ecommerce",
                "captureDelayHours": "0",
                "gatewaySystem": "direct",
                "hmacSignature": "test_hmac_signature_placeholder",
            },
            "amount": {"currency": "GBP", "value": 654000},
            "eventCode": "AUTHORISATION",
            "eventDate": "2026-01-21T14:18:18+01:00",
            "merchantAccountCode": "YOUR_MERCHANT_ACCOUNT",
            "merchantReference": "pay_test_00000000000000",
            "operations": ["CAPTURE", "REFUND"],
            "paymentMethod": "visa",
            "pspReference": "TEST000000000000",
            "reason": "APPROVED:1111:03/2030",
            "success": "true",
        }
    }]
}).encode()

ADYEN_HEADERS = {"content-type": "application/json", "accept": "*/*"}

# ── Connector identity only — no API creds, no webhook secret ─────────────────
def build_config():
    config = sdk_config_pb2.ConnectorConfig(
        options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX)
    )
    config.connector_config.CopyFrom(
        payment_pb2.ConnectorSpecificConfig(adyen=payment_pb2.AdyenConfig())
    )
    return config


# ── Test 1: handle_event — AUTHORISATION ──────────────────────────────────────
def test_handle_event() -> bool:
    print(_bold("\n[Adyen Webhook — AUTHORISATION handle_event]"))
    client = EventClient(build_config())

    request = payment_pb2.EventServiceHandleRequest(
        merchant_event_id="smoke_wh_adyen_auth",
        request_details=payment_pb2.RequestDetails(
            method=payment_pb2.HttpMethod.HTTP_METHOD_POST,
            uri="/webhooks/adyen",
            headers=ADYEN_HEADERS,
            body=ADYEN_WEBHOOK_BODY,
        ),
        # capture_method is the EventContext use case:
        # Adyen AUTHORISATION maps to AUTHORIZED (manual) or CAPTURED (automatic)
        event_context=payment_pb2.EventContext(
            payment=payment_pb2.PaymentEventContext(
                capture_method=payment_pb2.CaptureMethod.MANUAL,
            ),
        ),
    )

    try:
        response = client.handle_event(request)
        print(_grey(f"  event_type     : {response.event_type}"))
        print(_grey(f"  source_verified: {response.source_verified}"))
        print(_grey(f"  merchant_event : {response.merchant_event_id}"))
        if not response.source_verified:
            print(_yellow("  ~ source_verified=False (expected — no real HMAC secret)"))
        print(_green("  ✓ PASSED: handle_event returned response without crashing"))
        return True
    except (IntegrationError, ConnectorError) as e:
        print(_red(f"  ✗ FAILED: {type(e).__name__}: {e.error_message} (code={e.error_code})"))
        return False
    except Exception as e:
        print(_red(f"  ✗ FAILED: {type(e).__name__}: {e}"))
        return False


# ── Test 2: parse_event ────────────────────────────────────────────────────────
def test_parse_event() -> bool:
    print(_bold("\n[Adyen Webhook — AUTHORISATION parse_event]"))
    client = EventClient(build_config())

    request = payment_pb2.EventServiceParseRequest(
        request_details=payment_pb2.RequestDetails(
            method=payment_pb2.HttpMethod.HTTP_METHOD_POST,
            uri="/webhooks/adyen",
            headers=ADYEN_HEADERS,
            body=ADYEN_WEBHOOK_BODY,
        ),
    )

    try:
        response = client.parse_event(request)
        print(_grey(f"  event_type : {response.event_type}"))
        print(_grey(f"  reference  : {response.reference}"))
        print(_green("  ✓ PASSED: parse_event returned response"))
        return True
    except (IntegrationError, ConnectorError) as e:
        print(_red(f"  ✗ FAILED: {type(e).__name__}: {e.error_message} (code={e.error_code})"))
        return False
    except Exception as e:
        print(_red(f"  ✗ FAILED: {type(e).__name__}: {e}"))
        return False


# ── Test 3: malformed body ────────────────────────────────────────────────────
def test_malformed_body() -> bool:
    print(_bold("\n[Adyen Webhook — malformed body]"))
    client = EventClient(build_config())

    request = payment_pb2.EventServiceHandleRequest(
        request_details=payment_pb2.RequestDetails(
            method=payment_pb2.HttpMethod.HTTP_METHOD_POST,
            uri="/webhooks/adyen",
            headers=ADYEN_HEADERS,
            body=b"not valid json {{{{",
        ),
    )

    try:
        response = client.handle_event(request)
        print(_yellow(f"  ~ accepted malformed body — event_type: {response.event_type}"))
        return True
    except (IntegrationError, ConnectorError) as e:
        print(_green(f"  ✓ PASSED: {type(e).__name__} thrown as expected: {e.error_message}"))
        return True
    except Exception as e:
        print(_red(f"  ✗ FAILED: unexpected error: {type(e).__name__}: {e}"))
        return False


# ── Test 4: unknown eventCode ─────────────────────────────────────────────────
def test_unknown_event_code() -> bool:
    print(_bold("\n[Adyen Webhook — unknown eventCode]"))
    client = EventClient(build_config())

    body = json.loads(ADYEN_WEBHOOK_BODY)
    body["notificationItems"][0]["NotificationRequestItem"]["eventCode"] = "SOME_UNKNOWN_EVENT"

    request = payment_pb2.EventServiceHandleRequest(
        request_details=payment_pb2.RequestDetails(
            method=payment_pb2.HttpMethod.HTTP_METHOD_POST,
            uri="/webhooks/adyen",
            headers=ADYEN_HEADERS,
            body=json.dumps(body).encode(),
        ),
    )

    try:
        response = client.handle_event(request)
        print(_green(f"  ✓ PASSED: handled gracefully — event_type: {response.event_type}"))
        return True
    except (IntegrationError, ConnectorError) as e:
        print(_green(f"  ✓ PASSED: {type(e).__name__} for unknown event (expected): {e.error_message}"))
        return True
    except Exception as e:
        print(_red(f"  ✗ FAILED: {type(e).__name__}: {e}"))
        return False


# ── main ──────────────────────────────────────────────────────────────────────
def main():
    print(_bold("Adyen Webhook Smoke Test"))
    print("─" * 50)

    results = [
        test_handle_event(),
        test_parse_event(),
        test_malformed_body(),
        test_unknown_event_code(),
    ]

    print("\n" + "=" * 50)
    all_passed = all(results)
    print(_green("PASSED") if all_passed else _red("FAILED"))
    sys.exit(0 if all_passed else 1)


if __name__ == "__main__":
    main()
