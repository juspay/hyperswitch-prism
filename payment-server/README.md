# Payment Server - Hyperswitch Prism Integration

A Python server application that routes payments through Shift4 (USD) and Fiuu (EUR) using the `hyperswitch-prism` SDK.

## Setup

```bash
cd payment-server
pip install -r requirements.txt
python app.py
```

The server starts on `http://localhost:8080`.

## API Endpoints

### POST /authorize
Authorize a payment. Routes USD to Shift4, EUR to Fiuu.

```bash
curl -X POST http://localhost:8080/authorize \
  -H "Content-Type: application/json" \
  -d '{
    "amount": 1000,
    "currency": "USD",
    "card_number": "4111111111111111",
    "card_exp_month": "03",
    "card_exp_year": "2030",
    "card_cvc": "737",
    "card_holder_name": "John Doe"
  }'
```

### POST /refund
Refund a previously authorized payment.

```bash
curl -X POST http://localhost:8080/refund \
  -H "Content-Type: application/json" \
  -d '{
    "connector_transaction_id": "<from authorize response>",
    "amount": 1000,
    "currency": "USD"
  }'
```

### POST /authorize-and-refund
End-to-end flow: authorize then refund in one call.

```bash
curl -X POST http://localhost:8080/authorize-and-refund \
  -H "Content-Type: application/json" \
  -d '{"amount": 1000, "currency": "USD"}'
```

### GET /health
Health check endpoint.

## Routing Logic

| Currency | Connector |
|----------|-----------|
| USD      | Shift4    |
| EUR      | Fiuu      |

## Credentials

Loaded from `/home/grace/creds.json` at startup.

## Known Issues

- **Fiuu connector bug**: The Fiuu connector in `hyperswitch-prism` v0.0.1 has a deserialization bug in the Rust core. The Authorize and Refund flows are missing `preprocess_response: true` in the macro, causing "Failed to deserialize connector response" errors. See `friction-log.md` for details.
