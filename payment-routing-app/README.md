# Payment Routing App

A Python server that routes payments to different connectors based on currency using the `hyperswitch-prism` SDK.

## Routing Rules

| Currency | Connector       |
|----------|----------------|
| USD      | Authorizedotnet |
| EUR      | Cybersource     |

## Setup

```bash
# Install the SDK
pip install hyperswitch-prism==0.0.1

# Note: The published package has a symbol mismatch in the FFI bindings.
# See the "Known Issues" section below for the workaround.
```

## Credentials

The app reads credentials from `/home/grace/creds.json`. The expected format:

- **Authorizedotnet**: `api_key` -> login name, `key1` -> transaction key
- **Cybersource**: `api_key` -> API key, `key1` -> merchant account, `api_secret` -> API secret

## Running the Server

```bash
python3 server.py [port]   # Default: 8080
```

## API Endpoints

### POST /authorize
Authorize a payment. Currency determines which connector is used.

```json
{
  "currency": "USD",
  "amount_minor": 1000,
  "card_number": "4111111111111111",
  "card_exp_month": "03",
  "card_exp_year": "2030",
  "card_cvc": "737",
  "card_holder_name": "John Doe",
  "capture_method": "AUTOMATIC"
}
```

### POST /refund
Refund a previously authorized payment.

```json
{
  "connector_transaction_id": "80053768414",
  "currency": "USD",
  "amount_minor": 1000
}
```

### GET /health
Returns routing configuration and server status.

## Running Tests

```bash
# Direct SDK tests (no server needed)
python3 test_payments.py

# HTTP server tests (start server first)
python3 test_payments.py --server
```

## Known Issues

1. **hyperswitch-prism 0.0.1 FFI symbol mismatch**: The published PyPI package has Python bindings that reference 21 FFI symbols not present in the bundled `.so` file. A patch script is needed to comment out these references before the library can be imported.

2. **Authorizedotnet sandbox refunds**: Authorize.net sandbox requires transactions to settle (~24 hours) before refunds can be processed. Immediate refund attempts return error code 54.

3. **Authorizedotnet refunds require card data**: The `connector_feature_data` field must include credit card details (number + expiration) for Authorizedotnet refunds, which is not documented.

4. **Cybersource requires billing address**: Unlike Authorizedotnet, Cybersource authorize requests fail silently (return status 0) without a populated billing address and email.
