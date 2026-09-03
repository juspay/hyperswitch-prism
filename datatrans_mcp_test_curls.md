# Datatrans Connector — Raw Curls (MCP, sandbox)

Plain paste-ready curls — no variables. Only replace `<base64(merchantId:password)>`
(and the example `transactionId`s after step 1).

**Scenario:** shopper charged **$120.00 (USD)**; merchant settles **€110.40 (EUR)** @ `0.92`.
Top-level `amount`/`currency` = merchant settlement leg · `mcp` = customer leg + quote echo.

Run order: **1 → 2 → 4 → 3 → 5** (capture before refund).

## 1. Authorize (card, no-3DS, MCP)

`POST /v1/transactions/authorize` (native-3DS / mandate CIT would use `POST /v1/transactions`)

```bash
curl -X POST "https://api.sandbox.datatrans.com/v1/transactions/authorize" \
  -H "Authorization: Basic <base64(merchantId:password)>" \
  -H "Content-Type: application/json" \
  -d '{
    "currency": "EUR",
    "amount": 11040,
    "refno": "pay_XDIpVoRzJES5mkFgKquX",
    "autoSettle": false,
    "card": {
      "number": "4000001000000018",
      "expiryMonth": "06",
      "expiryYear": "28",
      "cvv": "123",
      "type": "PLAIN"
    },
    "mcp": {
      "currency": "USD",
      "amount": 12000,
      "conversionRate": 0.92,
      "transactionDate": "2026-08-11T11:59:59Z",
      "retrievalReferenceNumber": "123456789012",
      "provider": "Planet",
      "userId": "999000017",
      "reasonIndicator": "MI"
    }
  }'
```

→ `201` with `{ "transactionId": "..." }` — use it in steps 2–4.

## 2. PSync

`GET /v1/transactions/{transactionId}` → `"status": "authorized"` (autoSettle=false)

```bash
curl -X GET "https://api.sandbox.datatrans.com/v1/transactions/250908145512903456" \
  -H "Authorization: Basic <base64(merchantId:password)>"
```

## 3. Refund (credit)

`POST /v1/transactions/{transactionId}/credit` — amount/currency = customer-facing refund leg. Returns a **new** `transactionId` for the credit → use in step 5.

```bash
curl -X POST "https://api.sandbox.datatrans.com/v1/transactions/250908145512903456/credit" \
  -H "Authorization: Basic <base64(merchantId:password)>" \
  -H "Content-Type: application/json" \
  -d '{
    "currency": "USD",
    "amount": 12000,
    "refno": "pay_XDIpVoRzJES5mkFgKquX-refund1"
  }'
```

## 4. Capture (settle)

`POST /v1/transactions/{transactionId}/settle` — top-level = settlement leg persisted from auth; slimmer `mcp` = customer capture amount only.

```bash
curl -X POST "https://api.sandbox.datatrans.com/v1/transactions/250908145512903456/settle" \
  -H "Authorization: Basic <base64(merchantId:password)>" \
  -H "Content-Type: application/json" \
  -d '{
    "currency": "EUR",
    "amount": 11040,
    "refno": "pay_XDIpVoRzJES5mkFgKquX",
    "mcp": {
      "currency": "USD",
      "amount": 12000
    }
  }'
```

→ `202` settled.

## 5. Refund sync

`GET /v1/transactions/{creditTransactionId}` → `type: "credit"`, `status: "settled"`

```bash
curl -X GET "https://api.sandbox.datatrans.com/v1/transactions/250908151422903789" \
  -H "Authorization: Basic <base64(merchantId:password)>"
```

---

- Auth: HTTP Basic `base64(merchantId:password)` (e.g. `echo -n "1100006530:yourpassword" | base64`)
- MIT/alias charges: same `POST /v1/transactions/authorize` with `"card": { "alias": "...", "expiryMonth": "06", "expiryYear": "28", "type": "PLAIN" }` + same `mcp` block.
- SetupMandate (zero-auth): `mcp` is not sent.
- Docs: <https://docs.datatrans.ch/docs/multi-currency-pricing.md>
