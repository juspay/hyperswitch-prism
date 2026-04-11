# Friction Log — hyperswitch-prism SDK Integration

## Executive Summary

Integrating PayPal and Cybersource via `hyperswitch-prism==0.0.1` revealed friction primarily in three areas:

1. **Missing FFI transformers** — The PayPal access token flow (`create_server_authentication_token`) has no FFI transformer, forcing a manual HTTP workaround.
2. **Protobuf type discovery** — `SecretString` is defined in `payment_methods_pb2`, not `payment_pb2`, with no documentation or error message guiding the developer.
3. **Connector-specific undocumented requirements** — Cybersource requires billing address fields and PayPal refund requires `connector_feature_data`, neither of which is documented or validated with clear error messages.

---

## Recommendations

### Critical

1. **Add FFI transformer for `create_server_authentication_token`**
   - Currently, the FFI module only exposes `authorize`, `refund`, `capture`, etc. PayPal (and likely other OAuth-based connectors) require an access token obtained via `MerchantAuthenticationClient.create_server_authentication_token`, but this fails at runtime because no `create_server_authentication_token_req_transformer` exists in `connector_service_ffi`.
   - **Impact**: Developers must implement PayPal OAuth2 token exchange manually via direct HTTP, bypassing the SDK entirely for this step. This defeats the purpose of a unified SDK.

2. **Document connector-specific field requirements**
   - Cybersource requires `billing_address` fields (`first_name`, `last_name`, `line1`, `city`, `state`, `zip_code`, `country_alpha2_code`) and `customer.email`. PayPal requires `state.access_token` on every request and `connector_feature_data` on refund requests.
   - The error from Cybersource (`MISSING_FIELD: orderInformation.billTo.locality`) maps to SDK field names non-obviously.
   - **Suggestion**: Per-connector example files should include all required fields populated, not just the minimal skeleton.

3. **Document `connector_feature_data` pass-through requirement**
   - PayPal refund fails with `IntegrationError: Missing required field: connector_meta_data` unless `connector_feature_data` from the authorize response is passed to the refund request.
   - This flow dependency is undocumented. The SDK should either handle this internally or document it prominently.

### Non-Critical

4. **Expose `SecretString` from `payment_pb2` or document where to import it**
   - `payment_pb2.SecretString` does not exist. The type lives in `payment_methods_pb2.SecretString` (or must be constructed via `ParseDict`). Since `PaypalConfig`, `CybersourceConfig`, etc. reference `SecretString` fields, developers naturally try `payment_pb2.SecretString` first.
   - **Suggestion**: Re-export `SecretString` from `payment_pb2` or add a note in examples.

5. **Numeric status codes in responses**
   - Responses return status as integers (e.g., `8` for `CHARGED`, `4` for `REFUND_SUCCESS`). While protobuf enums can be decoded via `PaymentStatus.Name(status)`, this is non-obvious.
   - **Suggestion**: Include status name strings in example output or add a helper method.

6. **Credential mapping is non-obvious**
   - `creds.json` uses `auth_type`, `api_key`, `key1`, `api_secret` — but the SDK configs use `client_id`, `client_secret`, `merchant_account`. The mapping (e.g., PayPal `key1` = `client_id`, `api_key` = `client_secret`) is not documented.
   - **Suggestion**: Add a mapping table in the README or creds.json comments.

---

## Detailed Log

### Issue 1: `SecretString` not found in `payment_pb2`

- **Description**: Attempted to build `PaypalConfig` using `payment_pb2.SecretString(value=...)` — raised `AttributeError: module 'payments.generated.payment_pb2' has no attribute 'SecretString'`.
- **Time wasted**: ~5 minutes
- **Resolution**: Discovered via protobuf descriptor introspection that `SecretString` is in `payment_methods_pb2`. Switched to `ParseDict` approach instead, which auto-resolves nested message types.

### Issue 2: PayPal access token flow missing from FFI

- **Description**: `MerchantAuthenticationClient.create_server_authentication_token()` fails with `AttributeError: module 'payments.generated.connector_service_ffi' has no attribute 'create_server_authentication_token_req_transformer'`.
- **Time wasted**: ~15 minutes
- **Resolution**: Listed all available FFI transformers — confirmed the auth token flow is not included. Implemented PayPal OAuth2 token exchange via direct `httpx` HTTP call to `https://api-m.sandbox.paypal.com/v1/oauth2/token`, then passed the token in the `state.access_token` field.

### Issue 3: Cybersource returns `PAYMENT_STATUS_UNSPECIFIED` with empty billing address

- **Description**: Cybersource authorize returned status `0` (UNSPECIFIED) with no transaction ID. Error inspection revealed: `MISSING_FIELD: orderInformation.billTo.locality, orderInformation.billTo.lastName, orderInformation.billTo.address1, orderInformation.billTo.country`.
- **Time wasted**: ~10 minutes
- **Resolution**: Added full billing address fields to the authorize request. Also discovered `country_alpha2_code` is an enum (not a `SecretString`), which required a different serialization format than other address fields.

### Issue 4: PayPal refund requires `connector_feature_data`

- **Description**: PayPal refund failed with `IntegrationError: Missing required field: connector_meta_data` despite providing all documented fields.
- **Time wasted**: ~10 minutes
- **Resolution**: Inspected the authorize response and found a `connector_feature_data` field containing PayPal-specific JSON (capture_id, psync_flow, etc.). Passing this field through to the refund request resolved the error.

### Issue 5: Credential key mapping

- **Description**: `creds.json` uses generic keys (`api_key`, `key1`, `api_secret`) while protobuf configs use connector-specific names (`client_id`, `client_secret`, `merchant_account`). PayPal's `key1` is `client_id` and `api_key` is `client_secret` (counter-intuitive). Cybersource's `key1` is `merchant_account`.
- **Time wasted**: ~5 minutes
- **Resolution**: Cross-referenced `auth_type` field with config class field names and verified by comparing with the smoke test's `_build_connector_config` function.

### Issue 6: `country_alpha2_code` type mismatch

- **Description**: Address field `country_alpha2_code` is an enum type (`CountryAlpha2`), not a `SecretString` like all other address fields. Using `{"value": "US"}` format caused `unhashable type: 'dict'` error.
- **Time wasted**: ~3 minutes
- **Resolution**: Changed to plain string format `"country_alpha2_code": "US"` (matching protobuf enum serialization via `ParseDict`).

---

## Assumptions

1. **Sandbox environment**: All testing uses `Environment.SANDBOX`. Production would require different credentials and potentially different base URLs.
   - **Validated**: PayPal sandbox URL confirmed working; Cybersource sandbox credentials from `connector_1` used successfully.

2. **Test card number**: Used `4111111111111111` (Visa test card) for both connectors.
   - **Validated**: Both PayPal and Cybersource accepted this card in sandbox mode.

3. **Credential structure**: Assumed `creds.json` PayPal credentials are at `paypal.connector_account_details` and Cybersource at `cybersource.connector_1.connector_account_details`.
   - **Validated**: Both paths exist and contain valid credentials.

4. **PayPal requires access token for every request**: Based on the example code always including `state.access_token`.
   - **Validated**: Authorize and refund both succeed with fresh access tokens; no caching implemented.

5. **Automatic capture**: Used `AUTOMATIC` capture method for both connectors, meaning authorize + capture happens in one call.
   - **Validated**: Both connectors return `CHARGED` status (8) confirming successful auto-capture.

---

## Reference

- **Friction log location**: `/home/grace/session-3-test/hyperswitch-prism/payment-server/friction-log.md`
- **Application code**: `/home/grace/session-3-test/hyperswitch-prism/payment-server/`
- **SDK examples**: `/home/grace/session-3-test/hyperswitch-prism/examples/paypal/paypal.py` and `/home/grace/session-3-test/hyperswitch-prism/examples/cybersource/cybersource.py`
- **Smoke test reference**: `/home/grace/session-3-test/hyperswitch-prism/sdk/python/smoke-test/test_smoke.py`
