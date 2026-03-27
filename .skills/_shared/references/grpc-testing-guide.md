# gRPC Testing Guide

This guide covers how to test a connector implementation end-to-end using grpcurl against
the running gRPC server. Testing is mandatory -- a passing `cargo build` only proves syntax;
grpcurl tests prove correctness.

This guide can be used as a **subagent prompt** for a dedicated testing agent.

---

## gRPC Service Map

Each flow maps to a specific gRPC service and method:

| Flow | Service | Method | Full path |
|------|---------|--------|-----------|
| Authorize | PaymentService | Authorize | `types.PaymentService/Authorize` |
| PSync | PaymentService | Get | `types.PaymentService/Get` |
| Capture | PaymentService | Capture | `types.PaymentService/Capture` |
| Void | PaymentService | Void | `types.PaymentService/Void` |
| Refund | PaymentService | Refund | `types.PaymentService/Refund` |
| RSync | RefundService | Get | `types.RefundService/Get` |
| SetupMandate | PaymentService | SetupRecurring | `types.PaymentService/SetupRecurring` |
| RepeatPayment | RecurringPaymentService | Charge | `types.RecurringPaymentService/Charge` |
| CreateAccessToken | MerchantAuthenticationService | CreateAccessToken | `types.MerchantAuthenticationService/CreateAccessToken` |
| CreateSessionToken | MerchantAuthenticationService | CreateSessionToken | `types.MerchantAuthenticationService/CreateSessionToken` |
| CreateOrder | PaymentService | CreateOrder | `types.PaymentService/CreateOrder` |
| CreateConnectorCustomer | CustomerService | Create | `types.CustomerService/Create` |
| PaymentMethodToken | PaymentMethodService | Tokenize | `types.PaymentMethodService/Tokenize` |
| IncomingWebhook | EventService | HandleEvent | `types.EventService/HandleEvent` |
| AcceptDispute | DisputeService | Accept | `types.DisputeService/Accept` |
| SubmitEvidence | DisputeService | SubmitEvidence | `types.DisputeService/SubmitEvidence` |
| DefendDispute | DisputeService | Defend | `types.DisputeService/Defend` |

---

## Step 1: Start the gRPC Server

```bash
# Kill any existing processes on ports 8000 and 8080
lsof -ti:8000 | xargs kill -9 2>/dev/null || true
lsof -ti:8080 | xargs kill -9 2>/dev/null || true
sleep 2

# Start the service in background
cargo run --bin grpc-server &

# Wait up to 120s for readiness
for i in $(seq 1 60); do sleep 2; curl -s http://localhost:8000/health && break; done

# Verify gRPC services are available
grpcurl -plaintext localhost:8000 list
```

If the service fails to start, check build errors and fix before proceeding.

---

## Step 2: Load Credentials

```bash
cat creds.json | jq '.{connector_name}'
```

Credentials are connector-specific. Map them to gRPC headers:

| creds.json field | gRPC header |
|-----------------|-------------|
| `api_key` | `-H 'x-api-key: <value>'` |
| `key1` | `-H 'x-key1: <value>'` |
| `api_secret` | `-H 'x-api-secret: <value>'` |
| `merchant_id` | `-H 'x-merchant-id: <value>'` |

Always include: `-H 'x-connector: {connector_name}'`

Only include headers that exist in creds.json. Do not guess or add unused headers.

---

## Step 3: Test Authorize

```bash
grpcurl -plaintext \
  -H 'x-connector: {connector_name}' \
  -H 'x-api-key: <from_creds>' \
  -d '{
    "request_ref_id": {"id": "test_{connector}_auth_001"},
    "amount": 1000,
    "minor_amount": 1000,
    "currency": "USD",
    "webhook_url": "https://example.com/webhook",
    "payment_method": {
      "card": {
        "card_number": {"value": "4111111111111111"},
        "card_exp_month": {"value": "12"},
        "card_exp_year": {"value": "2030"},
        "card_holder_name": {"value": "John Doe"},
        "card_cvc": {"value": "123"}
      }
    },
    "email": {"value": "test@example.com"},
    "address": {
      "billing_address": {
        "first_name": {"value": "John"},
        "last_name": {"value": "Doe"},
        "line1": {"value": "123 Test St"},
        "city": {"value": "Test City"},
        "state": {"value": "CA"},
        "zip_code": {"value": "12345"},
        "country_alpha2_code": "US"
      }
    },
    "capture_method": "AUTOMATIC",
    "auth_type": "NO_THREE_DS",
    "enrolled_for_3ds": false,
    "return_url": "https://example.com/return"
  }' \
  localhost:8000 \
  types.PaymentService/Authorize
```

**Adapt per connector:**
- Replace card data with connector's sandbox test card numbers
- Change currency/country to match connector's supported regions
- Add `merchant_account_metadata` if connector requires it (check tech spec)
- For non-card payment methods, replace the `payment_method` object accordingly

---

## Step 4: Test PSync (Payment Status)

Use the `connector_transaction_id` from the Authorize response:

```bash
grpcurl -plaintext \
  -H 'x-connector: {connector_name}' \
  -H 'x-api-key: <from_creds>' \
  -d '{
    "request_ref_id": {"id": "test_{connector}_psync_001"},
    "connector_transaction_id": "<id_from_authorize_response>"
  }' \
  localhost:8000 \
  types.PaymentService/Get
```

---

## Step 5: Test Capture

```bash
grpcurl -plaintext \
  -H 'x-connector: {connector_name}' \
  -H 'x-api-key: <from_creds>' \
  -d '{
    "request_ref_id": {"id": "test_{connector}_capture_001"},
    "connector_transaction_id": "<id_from_authorize_response>",
    "amount": 1000,
    "minor_amount": 1000,
    "currency": "USD"
  }' \
  localhost:8000 \
  types.PaymentService/Capture
```

---

## Step 6: Test Refund

```bash
grpcurl -plaintext \
  -H 'x-connector: {connector_name}' \
  -H 'x-api-key: <from_creds>' \
  -d '{
    "request_ref_id": {"id": "test_{connector}_refund_001"},
    "connector_transaction_id": "<id_from_authorize_response>",
    "refund_amount": 1000,
    "minor_refund_amount": 1000,
    "currency": "USD"
  }' \
  localhost:8000 \
  types.PaymentService/Refund
```

---

## Step 7: Test RSync (Refund Status)

```bash
grpcurl -plaintext \
  -H 'x-connector: {connector_name}' \
  -H 'x-api-key: <from_creds>' \
  -d '{
    "request_ref_id": {"id": "test_{connector}_rsync_001"},
    "connector_transaction_id": "<refund_id_from_refund_response>"
  }' \
  localhost:8000 \
  types.RefundService/Get
```

---

## Step 8: Test Void

```bash
grpcurl -plaintext \
  -H 'x-connector: {connector_name}' \
  -H 'x-api-key: <from_creds>' \
  -d '{
    "request_ref_id": {"id": "test_{connector}_void_001"},
    "connector_transaction_id": "<id_from_authorize_response>"
  }' \
  localhost:8000 \
  types.PaymentService/Void
```

Note: Void requires an authorized-but-not-captured payment. Run Authorize with
`"capture_method": "MANUAL"` first, then Void that transaction.

---

## Validating Test Results

### PASS criteria (ALL must be true):
- No `Error invoking method` or `Failed to` in output
- Response contains valid JSON with a `status` field
- `status_code` is 2xx (200-299)
- Status is one of: `authorized`, `PENDING`, `charged`, `REQUIRES_CUSTOMER_ACTION`
- No `error` or `errorMessage` field (or error field is null/empty)

### FAIL indicators (ANY means test failed):
- `Error invoking method` -- grpcurl itself failed (wrong field, wrong method, connection refused)
- `status_code` not 2xx -- connector rejected the request
- `PAYMENT_FLOW_ERROR`, `INTERNAL`, `UNIMPLEMENTED`, `UNKNOWN` -- server error
- `"status": "failed"` or `"status": "FAILURE"`
- Non-null `error` object with a message
- No JSON response (empty output, timeout, crash)

---

## Build-Test Loop (Anti-Loop Safeguards)

When a test fails, you MUST fix the code and rebuild before retrying:

```
1. Build: cargo build --package connector-integration
2. If build fails -> read error -> fix code -> go to 1
3. Start service (if not running) -> load creds -> run grpcurl test
4. If test fails -> read SERVER LOGS -> identify root cause -> fix code -> go to 1
5. If credential error -> ask user for correct creds -> go to 3
6. Both pass -> SUCCESS
```

**Hard rules:**
- NEVER rerun grpcurl without changing code first. Same code = same result.
- 3-strike rule: same error 3 times = FAILED immediately
- Maximum 7 total loop iterations = FAILED regardless
- Always read server logs (not just grpcurl output) to diagnose errors
- Maintain a fix log: (1) error seen, (2) file changed, (3) what and why

### Error Classification

| Type | Signs | Action |
|------|-------|--------|
| gRPC config | Connection refused, wrong method | Fix grpcurl command, check proto |
| Credentials | 401/403, "unauthorized" | Ask user for correct creds |
| Request format | 400/422, "missing field" | Check server logs, fix request struct in transformers.rs, rebuild |
| Response parsing | Deserialization error, panic | Check server logs, fix response struct, rebuild |
| Server error | 500, PAYMENT_FLOW_ERROR | Check server logs for root cause, fix connector code, rebuild |

---

## Subagent Prompt Template

Use this to delegate testing to a separate subagent after implementation:

```
Test the {ConnectorName} connector's {FlowName} flow via grpcurl.

## Context
- Connector: {connector_name}
- Credentials: creds.json (field: {connector_name})
- Tech spec: grace/rulesbook/codegen/references/{connector}/technical_specification.md
- Testing guide: .skills/new-connector/references/grpc-testing-guide.md
- Connector source: crates/integrations/connector-integration/src/connectors/{connector}.rs
- Transformers: crates/integrations/connector-integration/src/connectors/{connector}/transformers.rs

## Instructions
1. Read the testing guide at .skills/new-connector/references/grpc-testing-guide.md
2. Start the gRPC server if not running
3. Load credentials from creds.json
4. Run the grpcurl test for {FlowName} using the correct service/method from the guide
5. Validate the response against PASS/FAIL criteria
6. If FAILED: read server logs, diagnose root cause, fix code, rebuild, retest
7. Follow anti-loop safeguards (3-strike rule, max 7 iterations, always change code between retries)
8. Report: PASS or FAIL with details, grpcurl output, and fix log if applicable
```
