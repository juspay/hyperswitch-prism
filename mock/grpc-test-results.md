# Direct grpcurl matrix — results

End-to-end verification of `mock-dummy` (v1) against the UCS gRPC server using direct `grpcurl` (no harness), with `data/field_probe/dummy.json` as the request-body source.

- **Date**: 2026-05-19
- **Branch**: `feat/dummy-connector`
- **HEAD**: `403dbfb3b` (after the two bug-fixes found during this run)
- **mock-dummy**: `127.0.0.1:8777` (with `MOCK_DUMMY_PUBLIC_URL=http://127.0.0.1:8777`)
- **grpc-server**: `0.0.0.0:8000` (with `CS__CONNECTORS__DUMMY__BASE_URL=http://127.0.0.1:8777/dummy/`)
- **Result**: **26/26 cells PASS** (after fixes). Two real connector-compat bugs in mock-dummy were caught and fixed mid-run.
- **Reproduce**: `bash mock/grpc-test-commands.sh`

## Required headers on every gRPC call

```
x-connector:   dummy
x-auth:        header-key
x-api-key:     sk_test_dummy
x-merchant-id: m_grpcurl_run
x-tenant-id:   default
x-request-id:  <unique per call>
```

The Dummy connector accepts any non-empty Bearer token in test mode; the mock rejects `sk_live_*` prefixes.

---

## A. Authorize sweep — every v1 payment method

| Cell | Payment method | Override | Status (RPC body) | Notable response field | Verdict |
|---|---|---|---|---|---|
| A1 | Card success | `card_number = 4242424242424242` | `CHARGED` | `connectorTransactionId: pi_...` | ✅ |
| A2 | Card decline | `card_number = 4000000000000002` | `FAILURE` | `error.connectorDetails.message: "Your card was declined."` (contains `declin`) | ✅ |
| A3 | Card 3DS | `card_number = 4000003800000446` | `AUTHENTICATION_PENDING` | `redirectionData.form.endpoint: http://127.0.0.1:8777/dummy/redirect/att_...` | ✅ (after Fix 1) |
| A4 | UPI success | `vpa_id = success@upi` | `CHARGED` | — | ✅ |
| A5 | UPI failure | `vpa_id = failure@upi` | `FAILURE` | `error.connectorDetails.code: "upi_declined"`; message: `"UPI collect declined."` | ✅ |
| A6 | Bancontact (`bancontact_card`) | — | `AUTHENTICATION_PENDING` | redirect URL present | ✅ |
| A7 | iDeal (`ideal`) | — | `AUTHENTICATION_PENDING` | redirect URL present | ✅ |
| A8 | Trustly (`trustly`) | — | `AUTHENTICATION_PENDING` | redirect URL present | ✅ |
| A9 | Blik (`blik`) | — | `AUTHENTICATION_PENDING` | redirect URL present | ✅ |
| A10 | MbWay (`mb_way`) | — | `AUTHENTICATION_PENDING` | redirect URL present | ✅ |
| A11 | Satispay (`satispay`) | — | `AUTHENTICATION_PENDING` | redirect URL present | ✅ |
| A12 | Wero (`wero`) | — | `AUTHENTICATION_PENDING` | redirect URL present | ✅ |
| A13 | AliPay Redirect (`ali_pay_redirect`) | — | `AUTHENTICATION_PENDING` | redirect URL present | ✅ |
| A14 | WeChat Pay QR (`we_chat_pay_qr`) | — | `AUTHENTICATION_PENDING` | redirect URL present | ✅ |
| A15 | Revolut Pay (`revolut_pay`) | — | `AUTHENTICATION_PENDING` | redirect URL present | ✅ |

### Sample raw responses

**A1 Card success**:
```json
{
  "merchantTransactionId":  "pi_76c1e7ccee1248989d2239df1b011c4e",
  "connectorTransactionId": "pi_76c1e7ccee1248989d2239df1b011c4e",
  "status": "CHARGED",
  "statusCode": 200
}
```

**A2 Card decline** (preserves `no3ds_fail_payment` fixture):
```json
{
  "merchantTransactionId":  "pi_46ee70ab00d2482dadd51c709c46efb2",
  "connectorTransactionId": "pi_46ee70ab00d2482dadd51c709c46efb2",
  "status": "FAILURE",
  "error": {
    "issuerDetails":   { "message": "card_declined", "networkDetails": { "errorMessage": "card_declined" } },
    "connectorDetails": {
      "code":    "card_declined",
      "message": "Your card was declined.",
      "reason":  "message - Your card was declined., decline_code - card_declined",
      "connectorTransactionId": "pi_46ee70ab00d2482dadd51c709c46efb2"
    }
  },
  "statusCode": 200
}
```

**A3 Card 3DS** (after Fix 1):
```json
{
  "merchantTransactionId":  "pi_568b0bf089f34eb6a0746f22737fc8c9",
  "connectorTransactionId": "pi_568b0bf089f34eb6a0746f22737fc8c9",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200,
  "redirectionData": {
    "form": {
      "endpoint":   "http://127.0.0.1:8777/dummy/redirect/att_b9d252cf77304d608263714703cad226",
      "method":     "HTTP_METHOD_GET",
      "formFields": {}
    }
  }
}
```

**A4 UPI success** (preserves UPI Collect baseline):
```json
{
  "merchantTransactionId":  "pi_f2d30c9f52384928bbdb9216ba37c157",
  "connectorTransactionId": "pi_f2d30c9f52384928bbdb9216ba37c157",
  "status": "CHARGED",
  "statusCode": 200
}
```

---

## B. Manual capture flow (Card 4242)

| Cell | Step | gRPC method | Status | Verdict |
|---|---|---|---|---|
| B1 | Authorize MANUAL | `PaymentService/Authorize` | `AUTHORIZED` | ✅ |
| B2 | PSync | `PaymentService/Get` | `AUTHORIZED` | ✅ |
| B3 | Capture (full) | `PaymentService/Capture` | `CHARGED` | ✅ |
| B4 | PSync after capture | `PaymentService/Get` | `CHARGED` | ✅ |
| B5 | Refund (full) | `PaymentService/Refund` | `REFUND_SUCCESS` | ✅ (after Fix 2) |
| B6 | RefundSync | `RefundService/Get` | `REFUND_SUCCESS` | ✅ |

```text
B1: AUTHORIZED   ctid=pi_dd35abf7bd3844bdb6771d2bcd7bc81b
B2: AUTHORIZED
B3: CHARGED
B4: CHARGED
B5: REFUND_SUCCESS  connectorRefundId=re_69d4282c6ddd4a0f99e5007ed54fb45e
B6: REFUND_SUCCESS
```

---

## C. Void flow (Card 4242)

| Cell | Step | gRPC method | Status | Verdict |
|---|---|---|---|---|
| C1 | Authorize MANUAL (separate PI) | `Authorize` | `AUTHORIZED` | ✅ |
| C2 | Void | `PaymentService/Void` | `VOIDED` | ✅ |
| C3 | PSync after void | `Get` | `VOIDED` | ✅ |

```text
C1: AUTHORIZED  ctid=pi_602b85045db04981b2e9110243000d72
C2: VOIDED
C3: VOIDED
```

---

## D. Redirect happy completion (Bancontact)

| Cell | Step | Method | Result | Verdict |
|---|---|---|---|---|
| D1 | Bancontact authorize | `Authorize` | `AUTHENTICATION_PENDING` + redirect URL | ✅ |
| D2 | Visit redirect URL | `curl GET` | HTTP `200` | ✅ |
| D3 | PSync after redirect | `Get` | `CHARGED` | ✅ |

```text
D1: AUTHENTICATION_PENDING   ctid=pi_2aab8a6a1ffc45d59a861329459d5715
    redirect=http://127.0.0.1:8777/dummy/redirect/att_45e878dc49a943c9a53ba9f48821a07d
D2: HTTP 200
D3: CHARGED
```

---

## E. Redirect-rejection path (iDeal, `?reject=1`)

| Cell | Step | Method | Result | Verdict |
|---|---|---|---|---|
| E1 | iDeal authorize | `Authorize` | `AUTHENTICATION_PENDING` + redirect URL | ✅ |
| E2 | Visit redirect with `?reject=1` | `curl GET` | HTTP `200` | ✅ |
| E3 | PSync after reject | `Get` | `FAILURE`, code `redirect_rejected`, message `"User rejected at redirect page."` | ✅ |

```text
E1: AUTHENTICATION_PENDING   ctid=pi_03a451dfe9fc415390d71a93a8cb7722
    redirect=http://127.0.0.1:8777/dummy/redirect/att_86150c1466294dcf95bd74a7ecfb6457
E2: HTTP 200
E3: FAILURE  code=redirect_rejected  msg="User rejected at redirect page."
```

---

## F. Admin webhook trigger

`POST /dummy/admin/trigger-webhook` is a mock-backend admin endpoint (HTTP, not gRPC). Verified separately.

| Cell | Scenario | Outcome | Verdict |
|---|---|---|---|
| F1 | Trigger payment_intent.succeeded → live Python sink on `127.0.0.1:9004` | mock returns `{delivered_to, status: 200, event_id: evt_*}`; sink received Stripe-shaped event body | ✅ |
| F2 | Trigger to unreachable target `http://127.0.0.1:1/never` | mock returns HTTP `502` with `{error: {type: "api_error", code: "webhook_delivery_failed", message: ...}}` | ✅ |

**F1 sink-received event body** (Stripe-shaped):
```json
{
  "id":       "evt_cf53397da6514b3cbf6d2616899dcaad",
  "object":   "event",
  "type":     "payment_intent.succeeded",
  "created":  1779178568,
  "livemode": false,
  "data": {
    "object": {
      "id":             "pi_fb4b1458ea4c43a68d1a2cf0c265f04d",
      "object":         "payment_intent",
      "status":         "succeeded",
      "amount":         1000,
      "currency":       "USD",
      "capture_method": "automatic",
      "client_secret":  "pi_fb4b1458ea4c43a68d1a2cf0c265f04d_secret_...",
      "created":        1779178568,
      "metadata":       { "order_id": "probe_txn_001" },
      "next_action":    null,
      "last_payment_error": null,
      "latest_charge": {
        "id":              "ch_c678ac7f2a0b4f928a000012411a4296",
        "object":          "charge",
        "amount":          1000,
        "amount_captured": 1000,
        "captured":        true,
        "paid":            true,
        "payment_intent":  "pi_fb4b1458ea4c43a68d1a2cf0c265f04d",
        "status":          "succeeded"
      }
    }
  }
}
```

---

## Bugs found and fixed during this run

Both bugs were silently swallowed by the Dummy connector's strict deserialization. The mock-dummy's own `curl` smoke tests passed because they bypass the connector entirely. **Only direct gRPC traffic surfaces these mismatches.**

### Fix 1 — `redirect_to_url.return_url` must be non-null String (commit `b03e16c12`)

Symptom: A3 / A6-A15 returned `AUTHENTICATION_PENDING` but `redirectionData` was empty.

Root cause: mock-dummy emitted `"return_url": null`. Dummy connector's `DummyRedirectToUrlResponse.return_url: String` (required, not `Option`). Serde failed and the connector's `Wrapper::deserialize(...).map_or(NoNextActionBody, ...)` swallowed the error.

Fix: change `RedirectToUrl.return_url` to `String`; thread `req.return_url` through `requires_action()` with `"about:blank"` fallback.

### Fix 2 — `Refund` response must include `metadata` field (commit `403dbfb3b`)

Symptom: B5 Refund returned `Internal / RESPONSE_DESERIALIZATION_FAILED` via gRPC.

Root cause: mock-dummy's refund response omitted `metadata`. Dummy connector's `RefundResponse.metadata: DummyMetadata` (required, not `Option`).

Fix: add `metadata: serde_json::Value` to mock-dummy's `Refund` struct, emit `{}`.

---

## Reproduction

1. Build: `cargo build -p mock-dummy --release && cargo build -p grpc-server`
2. Boot mock: `DUMMY_BACKEND_BIND=127.0.0.1:8777 MOCK_DUMMY_PUBLIC_URL=http://127.0.0.1:8777 ./target/release/mock-dummy &`
3. Boot grpc-server: `CS__CONNECTORS__DUMMY__BASE_URL=http://127.0.0.1:8777/dummy/ ./target/debug/grpc-server &`
4. Run: `bash mock/grpc-test-commands.sh`
