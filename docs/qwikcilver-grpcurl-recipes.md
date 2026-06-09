# Qwikcilver — grpcurl recipes (UAE sandbox)

Paste-ready `grpcurl` commands for the Qwikcilver wallet flows against
prism on `localhost:8000`, exercising the UAE Pine Labs sandbox
(`qc3.qwikcilver.com`).

All three flows go through **`CompositePaymentService`** — every
composite call automatically performs the Qwikcilver `/authorize`
session-login bootstrap and returns the resulting token in
`accessTokenResponse` alongside the operation result. No separate
session-bootstrap RPC is needed.

Test wallet: `4999771007702947` (holder MR. PRASHANT M, phone `8904860486`).

Replace the proto paths with your local checkout if different from
`/Users/kanika.c/code/connector-services/hyperswitch-prism-2/...`.

---

## 1. Wallet Authorize UAE + Redeem (one composite call)

`CompositePaymentService.Authorize` runs **both** the Qwikcilver
`/authorize` session bootstrap **and** the wallet Redeem in a single
RPC. The response carries `accessTokenResponse` (the minted session JWT)
plus `authorizeResponse` (the Redeem outcome).

The example below debits `0.20 AED` from the test wallet. The
destination wallet number rides on the typed
`payment_method.qwikcilver_wallet_direct.wallet_number` (DIRECT WALLETS
section of the proto). Composite POSTs internally to
`/api/v2/authorize`, then to `/api/v2/wallet/{wallet}/REDEEM`.

```bash
grpcurl \
    -plaintext \
    -H 'x-connector':'qwikcilver' \
    -H 'x-connector-config':'{"config":{"Qwikcilver":{"bootstrap_bearer_token":"eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJjdXJyZW50QmF0Y2hOdW1iZXIiOiIxMDcxMzA3NiIsInRlcm1pbmFsSWQiOiJBRkctV0VCLVRFU1QwMiIsInVzZXJOYW1lIjoibmV3bWFuYWdlciIsInBhc3N3b3JkIjoid2VsY29tZSIsIm5iZiI6MTU5OTU0MTUxNywiZXhwIjoxNjMxMDc3NTE3LCJpYXQiOjE1OTk1NDE1MTcsImlzcyI6Imh0dHBzOi8vcXdpa2NpbHZlci5jb20vIn0.YVYDGrDHUF6wZnwck2WM_Y8i95bKEyUKzevc1IIzmnaOhguZQhCY83bgJahvnqknAVXW5XKo83UPhb7jorxs2A","terminal_id":"AFG-Juspay","username":"manager","password":"welcome"}}}' \
    -H 'x-merchant-id':'JUSPAYUAT' \
    -H 'x-request-id':'qc-redeem-001' \
    -H 'x-tenant-id':'public' \
    -emit-defaults \
    -proto '/Users/kanika.c/code/connector-services/hyperswitch-prism-2/crates/types-traits/grpc-api-types/proto/composite_services.proto' \
    -import-path '/Users/kanika.c/code/connector-services/hyperswitch-prism-2/crates/types-traits/grpc-api-types/proto' \
    -d '{"merchant_transaction_id":"qc-redeem-001","merchant_order_id":"qc-redeem-001","amount":{"minor_amount":20,"currency":"AED"},"auth_type":"NO_THREE_DS","capture_method":"AUTOMATIC","enrolled_for_3ds":false,"payment_method":{"qwikcilver_wallet_direct":{"wallet_number":{"value":"4999771007702947"}}},"address":{"shipping_address":{},"billing_address":{}},"test_mode":true}' \
    'localhost:8000' \
    types.CompositePaymentService/Authorize
```

**Live response** — captured against `qc3.qwikcilver.com` on
`2026-06-05T12:01:30Z`. Capture `authorizeResponse.connectorTransactionId`
and `authorizeResponse.connectorFeatureData.value`; you'll need them to
drive the Cancel Redeem flow below. The `accessTokenResponse` block is
the session JWT composite minted automatically.

> `expiresInSeconds: "1200"` reflects the conservative 20-minute TTL we
> expose to the framework. The upstream JWT itself is valid 7 days; we
> refresh much sooner to stay clear of ops-side revocation.

```json
{
  "accessTokenResponse": {
    "accessToken": {
      "value": "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJjdXJyZW50QmF0Y2hOdW1iZXIiOiIxNzMwNjE1MyIsInRlcm1pbmFsSWQiOiJBRkctSnVzcGF5IiwicmZkIjoiOTIyMzM3MjAzNjg1NDc3MjE4NSIsInVkdCI6IiIsImF1dGhUeXBlIjoiQkFTSUMiLCJuYmYiOjE3ODA2NjA4OTAsImV4cCI6MTc4MTI2NTY5MCwiaWF0IjoxNzgwNjYwODkwLCJpc3MiOiJodHRwczovL3F3aWtjaWx2ZXIuY29tLyJ9.SANLneRiYm0hOPBC4QgVCDA7mE6lnbKkUUQT-sVPaRPYbC6L4q7rhBPapQV6X4EWO1OkejqXuM3dl8PGSg3YSw"
    },
    "tokenType": "Bearer",
    "expiresInSeconds": "1200",
    "status": "OPERATION_STATUS_SUCCESS",
    "statusCode": 200
  },
  "authorizeResponse": {
    "merchantTransactionId": "qc-redeem-1780660887",
    "connectorTransactionId": "17306153:1129324760429976552",
    "status": "CHARGED",
    "statusCode": 200,
    "responseHeaders": {},
    "state": {
      "accessToken": {
        "token": {
          "value": "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJjdXJyZW50QmF0Y2hOdW1iZXIiOiIxNzMwNjE1MyIsInRlcm1pbmFsSWQiOiJBRkctSnVzcGF5IiwicmZkIjoiOTIyMzM3MjAzNjg1NDc3MjE4NSIsInVkdCI6IiIsImF1dGhUeXBlIjoiQkFTSUMiLCJuYmYiOjE3ODA2NjA4OTAsImV4cCI6MTc4MTI2NTY5MCwiaWF0IjoxNzgwNjYwODkwLCJpc3MiOiJodHRwczovL3F3aWtjaWx2ZXIuY29tLyJ9.SANLneRiYm0hOPBC4QgVCDA7mE6lnbKkUUQT-sVPaRPYbC6L4q7rhBPapQV6X4EWO1OkejqXuM3dl8PGSg3YSw"
        },
        "expiresInSeconds": "1200",
        "tokenType": "Bearer"
      }
    },
    "connectorFeatureData": {
      "value": "{\"batch_number\":17306153,\"transaction_id\":1129324760429976552,\"wallet_number\":\"4999771007702947\"}"
    }
  },
  "compositeStatus": "COMPLETED"
}
```

---

## 2. Wallet Add Card — Refund Topup (credit refund eCard)

Posts a credit to the wallet via Qwikcilver's
`/api/v2/wallet/{wallet}/card`. Discriminator
`refund_metadata.refund_type = "add_card"` selects this branch.
`refund_type` is required — there is intentionally no default, since the
two branches move money in opposite directions.
`card_program_name` is also required (Pine Labs provisions region-specific
program names, e.g. `"Blue Retail UAE Refund eCard"` for UAE).

```bash
grpcurl \
    -plaintext \
    -H 'x-connector':'qwikcilver' \
    -H 'x-connector-config':'{"config":{"Qwikcilver":{"bootstrap_bearer_token":"eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJjdXJyZW50QmF0Y2hOdW1iZXIiOiIxMDcxMzA3NiIsInRlcm1pbmFsSWQiOiJBRkctV0VCLVRFU1QwMiIsInVzZXJOYW1lIjoibmV3bWFuYWdlciIsInBhc3N3b3JkIjoid2VsY29tZSIsIm5iZiI6MTU5OTU0MTUxNywiZXhwIjoxNjMxMDc3NTE3LCJpYXQiOjE1OTk1NDE1MTcsImlzcyI6Imh0dHBzOi8vcXdpa2NpbHZlci5jb20vIn0.YVYDGrDHUF6wZnwck2WM_Y8i95bKEyUKzevc1IIzmnaOhguZQhCY83bgJahvnqknAVXW5XKo83UPhb7jorxs2A","terminal_id":"AFG-Juspay","username":"manager","password":"welcome"}}}' \
    -H 'x-merchant-id':'JUSPAYUAT' \
    -H 'x-request-id':'qc-addcard-001' \
    -H 'x-tenant-id':'public' \
    -emit-defaults \
    -proto '/Users/kanika.c/code/connector-services/hyperswitch-prism-2/crates/types-traits/grpc-api-types/proto/composite_services.proto' \
    -import-path '/Users/kanika.c/code/connector-services/hyperswitch-prism-2/crates/types-traits/grpc-api-types/proto' \
    -d '{"merchant_refund_id":"qc-addcard-001","connector_transaction_id":"unused-for-add-card","payment_amount":20,"refund_amount":{"minor_amount":10,"currency":"AED"},"reason":"refund topup test","refund_metadata":{"value":"{\"refund_type\":\"add_card\",\"wallet_number\":\"4999771007702947\",\"card_program_name\":\"Blue Retail UAE Refund eCard\"}"},"test_mode":true}' \
    'localhost:8000' \
    types.CompositePaymentService/Refund
```

**Live response** — captured against `qc3.qwikcilver.com` on
`2026-06-05T12:01:36Z`.

```json
{
  "accessTokenResponse": {
    "accessToken": {
      "value": "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJjdXJyZW50QmF0Y2hOdW1iZXIiOiIxNzMwNjE1NSIsInRlcm1pbmFsSWQiOiJBRkctSnVzcGF5IiwicmZkIjoiOTIyMzM3MjAzNjg1NDc3MjE4NSIsInVkdCI6IiIsImF1dGhUeXBlIjoiQkFTSUMiLCJuYmYiOjE3ODA2NjA4OTYsImV4cCI6MTc4MTI2NTY5NiwiaWF0IjoxNzgwNjYwODk2LCJpc3MiOiJodHRwczovL3F3aWtjaWx2ZXIuY29tLyJ9.F1ahgdViMBA7B1tRvMf5Wmmed4tZcg4y4N9Us28p2X06TrbeTOOZzfaxSRy5MapVhZHH5WyE0_A0s5Kal7PsiA"
    },
    "tokenType": "Bearer",
    "expiresInSeconds": "1200",
    "status": "OPERATION_STATUS_SUCCESS",
    "statusCode": 200
  },
  "refundResponse": {
    "connectorRefundId": "17306155:3946434030274086863",
    "status": "REFUND_SUCCESS",
    "statusCode": 200,
    "responseHeaders": {},
    "connectorTransactionId": "unused-for-add-card"
  }
}
```

The connector POSTs `{"IdempotencyKey","Amount","CardProgramName","Notes","InvoiceNumber"}` to `/wallet/4999771007702947/card`.

---

## 3. Wallet Cancel REDEEM UAE (reverse the prior Redeem)

Reverses the Redeem from step 2 via the same `Refund` RPC. Discriminator
`refund_metadata.refund_type = "cancel_redeem"` selects this branch.
The `original_batch_number` and `original_transaction_id` come from the
Redeem response (`connectorFeatureData.value`). The connector posts to
`/api/v2/wallet/{wallet}/CANCELREDEEM`.

Substitute the `batch_number` / `transaction_id` from your own Redeem
response (the values shown below are from the §1 live run), or leave them
out and let the connector parse them from the composite
`connector_transaction_id` (`"{batch}:{txn}"`).

```bash
grpcurl \
    -plaintext \
    -H 'x-connector':'qwikcilver' \
    -H 'x-connector-config':'{"config":{"Qwikcilver":{"bootstrap_bearer_token":"eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJjdXJyZW50QmF0Y2hOdW1iZXIiOiIxMDcxMzA3NiIsInRlcm1pbmFsSWQiOiJBRkctV0VCLVRFU1QwMiIsInVzZXJOYW1lIjoibmV3bWFuYWdlciIsInBhc3N3b3JkIjoid2VsY29tZSIsIm5iZiI6MTU5OTU0MTUxNywiZXhwIjoxNjMxMDc3NTE3LCJpYXQiOjE1OTk1NDE1MTcsImlzcyI6Imh0dHBzOi8vcXdpa2NpbHZlci5jb20vIn0.YVYDGrDHUF6wZnwck2WM_Y8i95bKEyUKzevc1IIzmnaOhguZQhCY83bgJahvnqknAVXW5XKo83UPhb7jorxs2A","terminal_id":"AFG-Juspay","username":"manager","password":"welcome"}}}' \
    -H 'x-merchant-id':'JUSPAYUAT' \
    -H 'x-request-id':'qc-cancel-001' \
    -H 'x-tenant-id':'public' \
    -emit-defaults \
    -proto '/Users/kanika.c/code/connector-services/hyperswitch-prism-2/crates/types-traits/grpc-api-types/proto/composite_services.proto' \
    -import-path '/Users/kanika.c/code/connector-services/hyperswitch-prism-2/crates/types-traits/grpc-api-types/proto' \
    -d '{"merchant_refund_id":"qc-cancel-001","connector_transaction_id":"17306153:1129324760429976552","payment_amount":20,"refund_amount":{"minor_amount":20,"currency":"AED"},"reason":"cancel redeem test","refund_metadata":{"value":"{\"refund_type\":\"cancel_redeem\",\"wallet_number\":\"4999771007702947\",\"original_batch_number\":17306153,\"original_transaction_id\":1129324760429976552}"},"test_mode":true}' \
    'localhost:8000' \
    types.CompositePaymentService/Refund
```

**Live response** — captured against `qc3.qwikcilver.com` on
`2026-06-05T12:01:33Z` (reverses the §1 Redeem with the matching
batch+txn pair).

```json
{
  "accessTokenResponse": {
    "accessToken": {
      "value": "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJjdXJyZW50QmF0Y2hOdW1iZXIiOiIxNzMwNjE1NCIsInRlcm1pbmFsSWQiOiJBRkctSnVzcGF5IiwicmZkIjoiOTIyMzM3MjAzNjg1NDc3MjE4NSIsInVkdCI6IiIsImF1dGhUeXBlIjoiQkFTSUMiLCJuYmYiOjE3ODA2NjA4OTMsImV4cCI6MTc4MTI2NTY5MywiaWF0IjoxNzgwNjYwODkzLCJpc3MiOiJodHRwczovL3F3aWtjaWx2ZXIuY29tLyJ9.Te-jUNljbUnhOJ85YOLQqGrnhLpzxoAnu3aqn2KZHpmoQ3ugJ3XqonzbXOS9lTFWFtg68RYGEDx2mMGJGZ5t5Q"
    },
    "tokenType": "Bearer",
    "expiresInSeconds": "1200",
    "status": "OPERATION_STATUS_SUCCESS",
    "statusCode": 200
  },
  "refundResponse": {
    "connectorRefundId": "17306154:4093093437397311721",
    "status": "REFUND_SUCCESS",
    "statusCode": 200,
    "responseHeaders": {},
    "connectorTransactionId": "17306153:1129324760429976552"
  }
}
```

The connector posts `{"OriginalBatchNumber":17306153,"OriginalTransactionId":1129324760429976552,"Notes":"cancel redeem test"}` to `/wallet/4999771007702947/CANCELREDEEM`.

---

## Quick reference — shared shapes

### `x-connector-config` payload

```json
{
  "config": {
    "Qwikcilver": {
      "bootstrap_bearer_token": "<long-lived bearer used only for /authorize>",
      "terminal_id":            "AFG-Juspay",
      "username":               "manager",
      "password":               "welcome"
    }
  }
}
```

### `refund_metadata` discriminator (Refund flow only)

| `refund_type`    | Qwikcilver endpoint                   | Extra fields                                                         |
| ---------------- | ------------------------------------- | -------------------------------------------------------------------- |
| `"add_card"`     | `POST /api/v2/wallet/{wn}/card`        | `card_program_name` (required — region-specific, e.g. `"Blue Retail UAE Refund eCard"` for UAE) |
| `"cancel_redeem"`| `POST /api/v2/wallet/{wn}/CANCELREDEEM`| `original_batch_number`, `original_transaction_id` (or composite `connector_transaction_id` of form `"{batch}:{txn}"` as fallback) |

`wallet_number` and `refund_type` are required in both branches. There is
no default for `refund_type` — the two branches move money in opposite
directions, so a wrong default would be a footgun.

### Composite vs non-composite

Every composite call performs the Qwikcilver `/authorize` bootstrap
internally and returns the resulting session JWT in `accessTokenResponse`.
Non-composite RPCs require the caller to bootstrap via
`MerchantAuthenticationService.CreateServerAuthenticationToken` first
and thread `state.access_token` into each subsequent request.

| Flow              | Composite RPC                              | Non-composite RPC          |
| ----------------- | ------------------------------------------ | -------------------------- |
| `/authorize` + Redeem | `CompositePaymentService.Authorize`     | `PaymentService.Authorize` |
| `/authorize` + Refund (Add Card / Cancel Redeem) | `CompositePaymentService.Refund` | `PaymentService.Refund`    |

### Server prerequisites

```bash
# server running on 127.0.0.1:8000
lsof -nP -iTCP:8000 -sTCP:LISTEN
# proto files reachable
ls /Users/kanika.c/code/connector-services/hyperswitch-prism-2/crates/types-traits/grpc-api-types/proto/composite_services.proto
# grpcurl + jq
command -v grpcurl jq
```
