# Friction Log - Hyperswitch Prism Payment Integration

**Date:** 2026-04-09
**Task:** Build a Python server integrating Shift4 (USD) and Fiuu (EUR) connectors with authorization and refund flows.
**SDK Version:** hyperswitch-prism==0.0.1

---

## Executive Summary

The integration experience with `hyperswitch-prism` was smooth for the Shift4 connector but entirely blocked for Fiuu due to a critical bug in the SDK's Rust core. The unified API design is excellent in concept - the same `PaymentClient.authorize()` / `PaymentClient.refund()` calls work across connectors. However, the Fiuu connector's response deserialization is broken, making it impossible to complete EUR payment flows.

### Key Friction Points by Pattern

1. **Connector-level bugs** - Fiuu connector has a critical deserialization bug (blocks all Fiuu operations)
2. **Credential mapping ambiguity** - No clear documentation mapping `creds.json` auth types to SDK config field names
3. **Connector-specific field requirements** - Required fields differ per connector (e.g., `webhook_url`, `customer.email`, `billing_address.first_name`) but are not documented clearly
4. **Async/sync interop** - The SDK is async-only, requiring careful event loop management in sync frameworks like Flask

---

## Recommendations

### Critical

1. **Fix Fiuu Authorize/Refund response deserialization**
   - **File:** `crates/integrations/connector-integration/src/connectors/fiuu.rs`
   - **Issue:** The Authorize macro (line ~427) and Refund macro (line ~566) are missing `preprocess_response: true`. The Void flow (line ~543) has it correctly. The Fiuu Direct API returns `key=value` line format, but without the preprocessor flag the SDK tries to deserialize it as JSON, causing `ConnectorError: Failed to deserialize connector response`.
   - **Fix:** Add `preprocess_response: true` to both the Authorize and Refund macro invocations, matching the Void flow pattern.

2. **Document connector-specific required fields**
   - Each connector has unique required fields beyond the base schema (e.g., Shift4 requires `billing_address.first_name`, Fiuu requires `webhook_url` and `customer.email`). These should be documented per-connector in the SDK reference or returned as clear validation errors.

### Non-Critical

3. **Document credential field mapping for all auth types**
   - The `creds.json` uses generic field names (`api_key`, `key1`, `api_secret`) with an `auth_type` discriminator. The SDK config uses connector-specific field names (`merchant_id`, `verify_key`, `secret_key` for Fiuu). This mapping is buried in Rust source code (`router_data.rs`). A lookup table in the SDK reference would save significant time.

4. **Provide a sync wrapper or document async usage patterns**
   - The SDK is async-only. When used in sync frameworks (Flask, Django without ASGI), developers must manage event loops carefully. A sync wrapper or documentation on common patterns (e.g., `asyncio.new_event_loop()` per request) would help. Notably, reusing a `PaymentClient` across multiple `_run_async` calls fails with "Event loop is closed" because the httpx connection pool is bound to the first event loop.

5. **Improve error messages for missing required fields**
   - When `billing_address.first_name` was missing for Shift4, the error "Missing required field: billing_address.first_name" was helpful. However, Fiuu's deserialization error gives no indication of what went wrong on the API side.

---

## Detailed Log

### Issue 1: Fiuu Connector - Complete Authorization/Refund Failure (CRITICAL)

**Description:** All Fiuu payment operations (Authorize, Refund) fail with `ConnectorError: Failed to deserialize connector response` regardless of currency (tested EUR, MYR), card data, or request parameters. The error occurs in the Rust FFI layer and cannot be worked around from Python.

**Time wasted:** ~45 minutes investigating, testing different currencies/parameters, tracing through Rust source code to identify root cause.

**Steps taken to resolve:**
1. Tested with EUR currency (project requirement) - failed
2. Tested with MYR currency (used in official Fiuu test suite) - failed
3. Added `customer.email` field (present in Rust test but not in Python example) - failed
4. Ran the official `examples/fiuu/fiuu.py` with real credentials - failed
5. Traced through Rust source: discovered `fiuu.rs` Authorize macro is missing `preprocess_response: true` while Void macro has it
6. Confirmed the Fiuu Direct API (`/RMS/API/Direct/1.4.0/index.php`) returns `key=value` line format responses, not JSON
7. Concluded this is a framework-level bug that cannot be fixed from the SDK consumer side

**Root cause:** In `crates/integrations/connector-integration/src/connectors/fiuu.rs`, the `macro_connector_implementation!` for Authorize (line ~427) and Refund (line ~566) do not include `preprocess_response: true`, while Void (line ~543) does. The preprocessor converts Fiuu's `key=value` response format to JSON before deserialization.

---

### Issue 2: Credential Mapping Not Documented

**Description:** The `creds.json` file uses generic auth fields (`auth_type: "signature-key"`, `api_key`, `key1`, `api_secret`), but the SDK config uses connector-specific field names. For Fiuu: `api_key` maps to `verify_key`, `key1` maps to `merchant_id`, `api_secret` maps to `secret_key`. This mapping is only discoverable by reading the Rust source code in `router_data.rs`.

**Time wasted:** ~15 minutes searching docs, examples, and eventually Rust source to find the mapping.

**Steps taken to resolve:**
1. Checked SDK README - no mapping table
2. Checked `examples/fiuu/fiuu.py` - credentials are commented out with `...` placeholders
3. Checked `llm/sdk-reference.md` - no auth type mapping
4. Found the mapping in `crates/types-traits/domain_types/src/router_data.rs` lines 2444-2457

---

### Issue 3: Shift4 Requires billing_address.first_name

**Description:** Shift4 authorization fails with "Missing required field: billing_address.first_name" if billing address is empty. The Shift4 Python example includes this field, but it's easy to miss since other connectors don't require it.

**Time wasted:** ~5 minutes (error message was clear).

**Steps taken to resolve:**
1. Initial request with empty `billing_address: {}` failed
2. Checked `examples/shift4/shift4.py` - saw `first_name` in billing address
3. Added `first_name` to request - worked

---

### Issue 4: Async Event Loop Management

**Description:** The SDK's `PaymentClient` uses httpx internally with connection pooling. When running multiple async operations (authorize then refund) using separate `asyncio.new_event_loop()` calls, the second call fails with "Event loop is closed" because the httpx client's connection pool is bound to the first loop.

**Time wasted:** ~10 minutes debugging and restructuring code.

**Steps taken to resolve:**
1. Initial approach: call `_run_async()` separately for authorize and refund in the `/authorize-and-refund` endpoint
2. Second call failed with "Event loop is closed"
3. Solution: wrap both operations in a single async function and run it in one event loop

---

### Issue 5: Fiuu Requires webhook_url and customer.email

**Description:** The Fiuu connector requires `webhook_url` on authorize/refund requests and `customer.email` on authorize requests. These are not required by Shift4 or the base schema. The Fiuu Python example (`examples/fiuu/fiuu.py`) includes `webhook_url` but not `customer.email`. The Rust test file (`fiuu_payment_flows_test.rs`) includes both.

**Time wasted:** ~10 minutes cross-referencing examples and tests.

**Steps taken to resolve:**
1. Noticed `webhook_url` in Fiuu example but not Shift4 example
2. Found `customer.email` only in Rust test file, not in Python example
3. Added both to Fiuu-specific request building

---

## Assumptions

1. **Credential mapping for Fiuu:** Assumed the `signature-key` auth type maps as `api_key->verify_key`, `key1->merchant_id`, `api_secret->secret_key` based on Rust source code in `router_data.rs`. **Validated** by reading the Rust source and confirming with test files.

2. **Shift4 uses header-key auth:** Assumed `api_key` from `creds.json` maps directly to `Shift4Config.api_key`. **Validated** by successful authorization calls.

3. **Test card number:** Assumed `4111111111111111` works for both connectors' sandbox environments. **Validated** for Shift4 (success). Could not validate for Fiuu due to deserialization bug.

4. **EUR currency for Fiuu:** The project requirement routes EUR to Fiuu, but Fiuu's test suite uses MYR (Malaysian Ringgit). Tested both currencies - both fail with the same deserialization error, confirming the issue is not currency-specific but a connector-level bug.

5. **Flask as server framework:** Chose Flask for simplicity. The async SDK required event loop management, but this is a standard pattern. An ASGI framework (FastAPI, Quart) would have been more natural but adds dependencies.

---

## Reference

- **Friction log file path:** `/home/grace/session-4-test/hyperswitch-prism/friction-log.md`
- **Application code:** `/home/grace/session-4-test/hyperswitch-prism/payment-server/app.py`
- **Fiuu bug location:** `crates/integrations/connector-integration/src/connectors/fiuu.rs` (lines ~427, ~566 - missing `preprocess_response: true`)
- **Credential mapping source:** `crates/types-traits/domain_types/src/router_data.rs` (lines ~2444-2457)
