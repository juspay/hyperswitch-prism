# Task Implementation Agent

You are the **sole owner** of implementing a single task from `tasks.json`. You handle everything end-to-end: analyzing the prompt, finding connector files, implementing changes, building, testing via grpcurl, and committing.

**First**: Read this file fully to understand all phases and rules before proceeding.

---

## Inputs

| Parameter | Description | Example |
|-----------|-------------|---------|
| `{CONNECTOR_NAME}` | Connector name | `nuvei` |
| `{PAYMENT_METHOD}` | Payment method | `card` |
| `{PAYMENT_METHOD_TYPE}` | Payment method type | `credit` |
| `{PROMPT}` | Task description/prompt | `Enable 3DS flow for Nuvei credit card payments...` |
| `{CREDENTIALS}` | Connector credentials JSON | `{ "auth_type": "SignatureKey", ... }` |
| `{BRANCH}` | Git branch | `feat/nuvei-3ds` |
| `{CUSTOM_PROMPT}` | Custom prompt from config (optional) | `3DS-specific guidelines...` |

---

## Phase 1: Analyze the Task

Understand what changes are needed based on the `{PROMPT}` and `{CUSTOM_PROMPT}` (if provided):

1. **Identify the goal**: What flow/feature is being requested?
   - 3DS/ThreeDS flow enablement?
   - New payment method support?
   - Response field mapping fixes?
   - Auth changes?

2. **Apply custom prompt guidance**: If `{CUSTOM_PROMPT}` is provided:
   - Read and understand the custom guidelines
   - Note any connector-specific or flow-specific requirements
   - Apply these guidelines during implementation

3. **Identify the target**: Which connector files need changes?
   - Main connector file: `crates/integrations/connector-integration/src/connectors/{connector}.rs`
   - Transformers: `crates/integrations/connector-integration/src/connectors/{connector}/transformers.rs`

---

## Phase 2: Find Connector Files

```bash
# Find the connector source files
find crates/integrations/connector-integration/src/connectors -iname "*{connector}*" -type f 2>/dev/null | head -20
```

Note the actual file paths. If connector files don't exist:
- If the connector doesn't exist at all -> return FAILED with reason "connector not found"

Store:
- `{CONNECTOR_MAIN_FILE}` — main connector file path
- `{CONNECTOR_TRANSFORMERS_FILE}` — transformers file path

---

## Phase 3: Read & Understand Current Implementation

Read the connector files to understand:
1. Current flows implemented (look at `create_all_prerequisites!` macro)
2. Current auth pattern
3. Existing payment methods supported
4. Current 3DS/ThreeDS implementation status

Also read relevant pattern guides from `grace/rulesbook/codegen/guides/patterns/`:
- If implementing 3DS: read `pattern_3ds_flow.md` or similar

---

## Phase 4: Implement Changes

Based on the `{PROMPT}` and any `{CUSTOM_PROMPT}`, make the necessary code changes:

### Pre-Implementation Checklist:
- [ ] Read custom prompt guidance (if provided)
- [ ] Identify specific flow/method patterns to follow
- [ ] Note any connector-specific requirements from custom prompts

### Common Changes:

**For 3DS Flow Enablement:**
1. Add `ThreeDS` to `create_all_prerequisites!` if not present
2. Create or update `ThreeDSRequest`/`ThreeDSResponse` structs in transformers
3. Implement `TryFrom` traits for 3DS data mapping
4. Add `macro_connector_implementation!` for 3DS flow
5. Map response fields correctly per connector's API
6. **Custom prompt considerations**: Follow any 3DS-specific guidance in `{CUSTOM_PROMPT}`

**For New Payment Methods:**
1. Add payment method to appropriate flow's request/response structs
2. Update transformers to handle the new payment method type
3. Add TryFrom implementations
4. **Custom prompt considerations**: Apply payment method guidelines from `{CUSTOM_PROMPT}`

**For Response Field Mapping:**
1. Update response structs to include missing fields
2. Fix TryFrom implementations to map fields correctly
3. Ensure status mapping is correct
4. **Custom prompt considerations**: Use any error code mappings from `{CUSTOM_PROMPT}`

### Rules:
- Always use macros (never manual `ConnectorIntegrationV2`)
- Use `RouterDataV2` not `RouterData`
- Use `domain_types` not `hyperswitch_*`
- Match existing code patterns exactly
- Apply custom prompt guidance where applicable

---

## Phase 5: Build & Test Loop

**This is a single loop. You cannot exit until both `cargo build` AND `grpcurl` tests pass.**

### Anti-loop safeguards:
- **NEVER rerun the same grpcurl or cargo build without changing code first**
- **3-strike rule**: Same error 3 times = FAILED
- **Maximum 7 total iterations**
- **Must read error output/server logs between retries**

### The Loop:

```
1. Build: cargo build --package connector-integration
2. If build fails -> read error -> fix code -> go to 1
3. Start service: cargo run --bin grpc-server &
4. Run grpcurl test with the appropriate payment method
5. If grpcurl fails -> read server logs -> fix code -> go to 1
6. Both pass -> exit loop
```

### grpcurl Test:

Use credentials from `{CREDENTIALS}`. Construct headers based on auth_type:
- `SignatureKey`: `-H 'x-api-key: <api_key>' -H 'x-key1: <key1>' -H 'x-api-secret: <api_secret>'`
- `BodyKey`: `-H 'x-api-key: <api_key>'`
- `HeaderKey`: `-H 'x-api-key: <api_key>'`

Example grpcurl for card payment:
```bash
grpcurl -plaintext \
  -H 'x-connector: {connector}' \
  -H 'x-auth: {auth_type}' \
  -H 'x-api-key: {api_key}' \
  -H 'x-key1: {key1}' \
  -H 'x-api-secret: {api_secret}' \
  -d '{
    "request_ref_id": {"id": "test_{connector}_001"},
    "amount": 100,
    "minor_amount": 10000,
    "currency": "USD",
    "payment_method": {
      "card": {
        "card_number": {"value": "4111111111111111"},
        "card_exp_month": {"value": "12"},
        "card_exp_year": {"value": "2025"},
        "card_cvc": {"value": "123"},
        "card_holder_name": {"value": "Test User"}
      }
    },
    "email": {"value": "test@example.com"},
    "capture_method": "AUTOMATIC",
    "auth_type": "THREE_DS",
    "enrolled_for_3ds": true,
    "return_url": "https://example.com/return"
  }' \
  localhost:8000 \
  ucs.v2.PaymentService/Authorize
```

**PASS criteria:**
- No `Error invoking method`
- Response has valid JSON with `"status"` field
- `status_code` is 2xx
- Status is one of: `authorized`, `PENDING`, `charged`, `REQUIRES_CUSTOMER_ACTION`

---

## Phase 6: Commit Changes

```bash
# Stage only connector-specific files
git add crates/integrations/connector-integration/src/connectors/{connector}*

# Commit with descriptive message
git commit -m "feat({connector}): {brief description of changes}

- {change 1}
- {change 2}

Task: {prompt_summary}"
```

---

## Phase 7: Report

**Return result:**

```
CONNECTOR: {connector_name}
PAYMENT_METHOD: {payment_method}
PAYMENT_METHOD_TYPE: {payment_method_type}
STATUS: SUCCESS | FAILED | SKIPPED
FILES_MODIFIED:
  - <path1>
  - <path2>
REASON: <if not SUCCESS, explain why>
```

**STATUS definitions:**
- **SUCCESS**: Build passed AND grpcurl passed AND code was committed
- **FAILED**: Any phase failed after attempting it
- **SKIPPED**: Connector files not found or task was invalid
