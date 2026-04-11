# Payment Server — PayPal & Cybersource Integration

A Python server application using `hyperswitch-prism` SDK to process payments through PayPal and Cybersource with currency-based routing.

## Routing Logic

| Currency | Connector    |
|----------|-------------|
| USD      | PayPal       |
| EUR      | Cybersource  |

## Setup

```bash
cd payment-server
pip install -r requirements.txt
```

Ensure credentials are available at `/home/grace/creds.json` (or set `CREDS_PATH` env var).

## Run the Server

```bash
python3 app.py
```

Server starts on `http://localhost:8080`.

## API Endpoints

### POST /authorize

Authorize a payment. Currency determines which connector is used.

```bash
# USD payment -> PayPal
curl -X POST http://localhost:8080/authorize \
  -H "Content-Type: application/json" \
  -d '{"amount_minor": 1000, "currency": "USD"}'

# EUR payment -> Cybersource
curl -X POST http://localhost:8080/authorize \
  -H "Content-Type: application/json" \
  -d '{"amount_minor": 1000, "currency": "EUR"}'
```

### POST /refund

Refund a previously authorized payment.

```bash
curl -X POST http://localhost:8080/refund \
  -H "Content-Type: application/json" \
  -d '{
    "connector_transaction_id": "<from authorize response>",
    "amount_minor": 1000,
    "currency": "USD",
    "connector_feature_data": "<from authorize response, if present>"
  }'
```

### GET /health

Health check endpoint.

## Run Tests

```bash
python3 test_payments.py
```

Tests authorize + refund flows for both USD/PayPal and EUR/Cybersource.

## Project Structure

```
payment-server/
  app.py               - Flask server with /authorize and /refund endpoints
  config.py            - Connector configuration from creds.json
  router.py            - Currency-based routing logic
  payment_service.py   - Payment authorization and refund flows
  test_payments.py     - Integration tests
  requirements.txt     - Python dependencies
```
