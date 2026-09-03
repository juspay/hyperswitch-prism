#!/usr/bin/env bash
# Datatrans sandbox — raw curls, no variables. Replace only the Basic auth string.
# Shopper charged $120.00 (USD); merchant settles €110.40 (EUR) @ 0.92.

# 1. Authorize (card, no-3DS, MCP)
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

# 2. PSync
curl -X GET "https://api.sandbox.datatrans.com/v1/transactions/250908145512903456" \
  -H "Authorization: Basic <base64(merchantId:password)>"

# 3. Refund (credit)
curl -X POST "https://api.sandbox.datatrans.com/v1/transactions/250908145512903456/credit" \
  -H "Authorization: Basic <base64(merchantId:password)>" \
  -H "Content-Type: application/json" \
  -d '{
    "currency": "USD",
    "amount": 12000,
    "refno": "pay_XDIpVoRzJES5mkFgKquX-refund1"
  }'

# 4. Capture (settle)
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

# 5. Refund sync
curl -X GET "https://api.sandbox.datatrans.com/v1/transactions/250908151422903789" \
  -H "Authorization: Basic <base64(merchantId:password)>"
