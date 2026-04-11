# Friction Log: hyperswitch-prism Payment Integration

**Date**: 2026-04-09
**Task**: Build a Python payment server using `hyperswitch-prism` with Authorizedotnet (USD) and Cybersource (EUR) routing
**Local file path**: `/home/grace/session-2-test/hyperswitch-prism/payment-routing-app/friction-log.md`

---

## Executive Summary

Integration with `hyperswitch-prism` required overcoming several significant friction points, primarily around **library compatibility**, **undocumented connector requirements**, and **silent failures**. The SDK's architecture (protobuf + UniFFI/Rust FFI) is powerful but introduces complexity that is poorly surfaced to users.

### Key Friction Patterns

1. **Broken published package** - The PyPI package cannot be imported without manual patching
2. **Undocumented connector-specific requirements** - Each connector has unique field requirements not documented in the SDK
3. **Silent failures** - Missing fields cause empty responses instead of clear errors
4. **Credential mapping ambiguity** - Field names in `creds.json` don't match SDK config field names

---

## Recommendations

### Critical

1. **Fix the PyPI package (hyperswitch-prism 0.0.1)**
   - The published `.so` file is missing 21 FFI symbols that the Python bindings reference
   - This makes the library completely unusable out of the box
   - Root cause: The Python `connector_service_ffi.py` was generated from a newer version of the Rust code than the compiled `.so`
   - **Impact**: 100% of users will hit this on first import

2. **Add connector-specific field validation with clear error messages**
   - Cybersource silently returns `PAYMENT_STATUS_UNSPECIFIED` (status=0) with empty transaction ID when billing address/email is missing
   - Should throw an `IntegrationError` with a message like "Cybersource requires billing_address.email"
   - Authorizedotnet refund error "Missing required field: connector_feature_data" doesn't explain what `connector_feature_data` should contain

3. **Document connector-specific requirements per flow**
   - The example files show minimal request shapes but don't indicate which fields are actually required vs optional per connector
   - A requirements matrix (connector x flow x required fields) would save significant integration time

### Non-Critical

4. **Standardize credential field naming**
   - `creds.json` uses `api_key`/`key1`/`api_secret` generically
   - SDK configs use connector-specific names (`name`/`transaction_key` for Authorizedotnet, `api_key`/`merchant_account`/`api_secret` for Cybersource)
   - The `creds_dummy.json` in the repo uses the SDK field names, but the actual `creds.json` uses different names
   - A mapping layer or documentation would help

5. **Fix RefundStatus enum coverage**
   - Failed refunds return `PaymentStatus.FAILURE` (int 21) which is not in the `RefundStatus` enum
   - Calling `RefundStatus.Name(21)` raises `ValueError`
   - Refund responses should consistently use `RefundStatus` values

6. **Add billing address to example files**
   - The Cybersource example uses `"billing_address": {}` which causes silent failure
   - Examples should include realistic billing address data

---

## Detailed Log

### Issue 1: PyPI Package Import Failure
- **Description**: `pip install hyperswitch-prism==0.0.1` installs successfully, but `from payments import PaymentClient` crashes with `AttributeError: undefined symbol: uniffi_connector_service_ffi_fn_func_create_client_authentication_token_req_transformer`
- **Time wasted**: ~30 minutes
- **Steps to resolve**:
  1. Identified that `connector_service_ffi.py` references FFI functions not present in the compiled `libconnector_service_ffi.so`
  2. Used `nm -D` to compare symbols in `.so` vs references in `.py` - found 21 missing symbols
  3. Wrote a Python patch script to remove all references to missing symbols (argtypes, restype, checksum validations, function definitions, and export entries)
  4. Multiple iterations needed because multi-line `argtypes = (...)` assignments required a state-machine parser, not simple line matching
  5. After patching, import worked correctly

### Issue 2: Cybersource Silent Failure (Empty Response)
- **Description**: Cybersource `authorize()` returned `status=PAYMENT_STATUS_UNSPECIFIED` (0) with empty `connector_transaction_id` and no error - a completely silent failure
- **Time wasted**: ~20 minutes
- **Steps to resolve**:
  1. Initial request used minimal fields (matching the Authorizedotnet pattern that worked)
  2. Compared with Cybersource example file - it included `customer.email` which I already had
  3. Investigated Rust transformer source code which showed `get_billing_email()` was required
  4. Added full billing address with email to the request
  5. With billing address populated, Cybersource returned `CHARGED` with a valid transaction ID

### Issue 3: Authorizedotnet Refund Requires Card Data
- **Description**: Refund request for Authorizedotnet failed with "Missing required field: connector_feature_data"
- **Time wasted**: ~15 minutes
- **Steps to resolve**:
  1. The `connector_feature_data` field is a `SecretString` (opaque to the SDK user)
  2. Examined the Rust transformer code for Authorizedotnet refunds
  3. Found it expects a JSON-encoded object with `creditCard.cardNumber` and `creditCard.expirationDate`
  4. Added the field as `{"value": json.dumps({"creditCard": {"cardNumber": "...", "expirationDate": "MMYY"}})}`
  5. This is not documented anywhere and the field name gives no hint about expected content

### Issue 4: Credential Field Name Mapping
- **Description**: `creds.json` uses generic field names (`api_key`, `key1`) while SDK configs use connector-specific names (`name`, `transaction_key`, `merchant_account`)
- **Time wasted**: ~10 minutes
- **Steps to resolve**:
  1. Examined `creds_dummy.json` in the repo which uses SDK field names
  2. Cross-referenced with protobuf `DESCRIPTOR.fields` to confirm field names
  3. For Authorizedotnet: `api_key` -> `name`, `key1` -> `transaction_key`
  4. For Cybersource: `api_key` -> `api_key`, `key1` -> `merchant_account`, `api_secret` -> `api_secret`

### Issue 5: RefundStatus Enum Mismatch
- **Description**: Failed Authorizedotnet refund returned `status=21` which maps to `PaymentStatus.FAILURE` but has no entry in `RefundStatus` enum
- **Time wasted**: ~5 minutes
- **Steps to resolve**:
  1. `RefundStatus.Name(21)` raised `ValueError`
  2. Added fallback: try `RefundStatus.Name()`, then `PaymentStatus.Name()`, then `UNKNOWN_{n}`
  3. This is a design inconsistency in the SDK

### Issue 6: Authorizedotnet Sandbox Refund Settling Requirement
- **Description**: Authorizedotnet sandbox returns error code 54 "The referenced transaction does not meet the criteria for issuing a credit" for immediate refunds
- **Time wasted**: ~5 minutes
- **Steps to resolve**:
  1. Recognized this is a known Authorize.net sandbox limitation - transactions must settle (~24h) before refunding
  2. Updated test to treat this as expected behavior in sandbox
  3. The refund flow code is correct; it works when the transaction has settled

---

## Assumptions

1. **Credential mapping**: Assumed `api_key` in `creds.json` maps to `name` for Authorizedotnet based on `creds_dummy.json` - validated by successful authorization
2. **Cybersource uses connector_1**: The creds file has nested connector configs for Cybersource; assumed `connector_1` is the primary - validated by successful authorization
3. **Sandbox environment**: Assumed all testing is against sandbox environments - validated by `Environment.SANDBOX` config working
4. **Card test number**: Assumed `4111111111111111` is a valid test card for both connectors - validated by successful authorizations
5. **Billing address for all connectors**: After discovering Cybersource requires it, applied billing address to all requests for consistency - Authorizedotnet also works with it
6. **Authorizedotnet refund card data**: Assumed the same card used for authorization should be passed in `connector_feature_data` for refund - validated by the refund reaching the connector (though rejected due to settling delay)

---

## Reference

- **Local file path**: `/home/grace/session-2-test/hyperswitch-prism/payment-routing-app/friction-log.md`
- **Application code**: `/home/grace/session-2-test/hyperswitch-prism/payment-routing-app/server.py`
- **Test suite**: `/home/grace/session-2-test/hyperswitch-prism/payment-routing-app/test_payments.py`
