# Friction Log — Payment Integration with hs-paylib

## Executive Summary

### Key Friction Points by Pattern

1. **Package Discovery & Naming**: The task specified `hs-paylib` as the library name, but no documentation within the repository references this name. The repo README and SDK docs refer to the package as `hyperswitch-prism`. The npm package is published as `hs-paylib` but this name appears nowhere in the source repo, creating confusion about which package to install.

2. **Platform-Specific Native Library**: The `hs-paylib` npm package ships only a macOS ARM64 `.dylib` file. Running on Linux requires building the Rust FFI library from source, which is a significant barrier for development and CI/CD.

3. **Credential Mapping Ambiguity**: The `creds.json` file uses generic field names (`api_key`, `key1`, `api_secret`, `auth_type`) that don't map directly to the SDK's typed config fields (`clientId`, `clientSecret`, `merchantAccount`). The mapping must be inferred from the SDK's README examples.

4. **Contradictory Routing Requirements**: The task specifies both currency-based routing (USD→Cybersource, EUR→PayPal) and amount-based routing (>100 USD→Cybersource, <100 USD→Adyen). These overlap in the USD case and Adyen was not mentioned in the initial connector list.

5. **Error Object Serialization**: The `response.error` field is a protobuf object with nested sub-objects (`unifiedDetails`, `connectorDetails`, `issuerDetails`) rather than simple `message`/`code`/`reason` fields as shown in the README examples.

6. **Undocumented Connector Requirements**: Cybersource requires `customer.email` and a full billing address (locality, lastName, address1, country) but the SDK README examples show `address: { billingAddress: {} }` as sufficient. PayPal refunds require `connectorFeatureData` from the authorize response, which is not documented in the refund flow examples.

---

## Recommendations

### Critical

1. **Publish platform-specific npm packages**: Ship pre-built native libraries for Linux x64, Linux ARM64, macOS x64, and macOS ARM64. The current macOS-only `.dylib` makes the package unusable on Linux without building from source.
   
2. **Align package name across documentation**: Either update the repo README to reference `hs-paylib` or rename the npm package to `hyperswitch-prism` to match the repo documentation. The mismatch wastes time during initial setup.

3. **Provide credential mapping documentation**: Add a clear mapping table showing how `creds.json` field names (`api_key`, `key1`, `api_secret`) correspond to SDK config fields (`apiKey`, `merchantAccount`, `clientId`) for each connector.

### Non-Critical

4. **Update README error handling examples**: The README shows `response.error?.message` and `response.error?.code` directly, but the actual type is `IErrorInfo` with nested `unifiedDetails` and `connectorDetails` objects. Update examples to match the actual API.

5. **Add Linux build instructions to SDK README**: Include steps for building the Rust FFI library on Linux, or document that the npm package currently only supports macOS.

6. **Clarify PayPal access token flow in Quick Start**: The PayPal flow requires a separate `MerchantAuthenticationClient` call before authorization, which is documented deep in the README but easy to miss.

7. **Document connector-specific required fields**: Cybersource requires email and full billing address; PayPal refunds require `connectorFeatureData` from the authorize response. These are runtime failures not caught by TypeScript types.

8. **Document `connectorFeatureData` pass-through pattern**: For connectors like PayPal, the authorize response's `connectorFeatureData` must be stored and passed to subsequent operations (refund, capture). This pattern is not documented anywhere.

---

## Detailed Log

### Issue 1: Package Name Confusion
- **Description**: Task specified `hs-paylib` as the library. The repository's `sdk/javascript/package.json` and README both use `hyperswitch-prism` as the package name. Running `npm install hyperswitch-prism` failed with `ETARGET` — the package doesn't exist on npm under that name. Only after running `npm search hs-paylib` was the correct published package name discovered.
- **Time wasted**: ~5 minutes
- **Resolution**: Used `npm search` to find the actual published package name `hs-paylib` on npm.

### Issue 2: Native Library Missing for Linux
- **Description**: After installing `hs-paylib`, running any code failed immediately with `Error: Failed to load shared library: cannot open shared object file: No such file or directory`. The npm package only contains `libconnector_service_ffi.dylib` (macOS ARM64 Mach-O binary). The SDK's `uniffi_client.ts` expects `libconnector_service_ffi.so` on Linux.
- **Time wasted**: ~15 minutes
- **Resolution**: Built the Rust FFI library from source using `cargo build --release -p ffi` in the hyperswitch-prism repo root. Copied the resulting `.so` file into the npm package's expected location.

### Issue 3: Credential Field Name Mapping
- **Description**: The `creds.json` uses connector-agnostic field names (`auth_type: "signature-key"`, `api_key`, `key1`, `api_secret`) while the SDK uses connector-specific field names (e.g., Cybersource: `apiKey`, `merchantAccount`, `apiSecret`; PayPal: `clientId`, `clientSecret`). For PayPal, `key1` maps to `clientId` and `api_key` maps to `clientSecret`, which is counter-intuitive.
- **Time wasted**: ~10 minutes
- **Resolution**: Cross-referenced the README's connector authentication section with the `creds.json` structure to determine the correct mapping for each connector.

### Issue 4: Contradictory Routing Rules
- **Description**: The task requirements state two routing rules that partially conflict:
  - "USD → Cybersource, EUR → PayPal" (connector routing)
  - "amount > 100 USD → Cybersource, amount < 100 USD → Adyen" (validation)
  Adyen was not listed as a required connector, yet appears in the validation criteria. The rules overlap for USD payments.
- **Time wasted**: ~5 minutes
- **Resolution**: Implemented combined routing: EUR→PayPal, USD >$100→Cybersource, USD ≤$100→Adyen. This satisfies both requirements.

### Issue 5: Error Object Structure Mismatch
- **Description**: The README shows `response.error?.message` and `response.error?.code` as direct properties, but the actual TypeScript interface `IErrorInfo` has nested objects: `unifiedDetails` (with `code`, `message`, `description`) and `connectorDetails` (with `code`, `message`, `reason`). Code that follows the README examples fails TypeScript compilation.
- **Time wasted**: ~5 minutes
- **Resolution**: Inspected the generated proto types to find the actual error structure and used `response.error.unifiedDetails?.message` etc.

### Issue 6: Cybersource Requires Email and Full Billing Address
- **Description**: Cybersource authorization failed with `MISSING_REQUIRED_FIELD: email` initially, then after adding email, failed again with `MISSING_FIELD` for `orderInformation.billTo.locality`, `lastName`, `address1`, and `country`. The SDK README shows `address: { billingAddress: {} }` as a valid minimal config, but Cybersource requires all these fields.
- **Time wasted**: ~8 minutes
- **Resolution**: Added full customer email and billing address with firstName, lastName, line1, city, state, zipCode, and countryAlpha2Code.

### Issue 7: PayPal Refund Requires connectorFeatureData
- **Description**: PayPal refund failed with `MISSING_REQUIRED_FIELD: connector_meta_data`. The authorize response includes a `connectorFeatureData` field containing PayPal-specific metadata (authorize_id, capture_id, psync_flow). This must be stored and passed back to the refund request. This pattern is completely undocumented in the SDK README.
- **Time wasted**: ~15 minutes
- **Resolution**: Inspected the Rust source code (`paypal/transformers.rs`) to discover the `PaypalMeta` struct and the `connectorFeatureData` pass-through requirement. Modified the API to return and accept this field.

### Issue 8: Protobuf null vs undefined
- **Description**: The protobuf-generated types use `string | null` for optional fields, but standard TypeScript interfaces use `string | undefined`. This causes type incompatibilities when passing protobuf field values to plain TypeScript interfaces, requiring explicit null-to-undefined conversions with `?? undefined`.
- **Time wasted**: ~3 minutes
- **Resolution**: Added nullish coalescing operators (`?? undefined`) when extracting fields from protobuf responses.

---

## Assumptions

1. **Package name**: Assumed `hs-paylib` is the correct npm package name based on npm search results, despite the repo documentation using `hyperswitch-prism`.
   - **Validated**: `npm install hs-paylib` succeeded and the package exports match the documented API.

2. **Credential mapping for PayPal**: Assumed `key1` in creds.json maps to `clientId` and `api_key` maps to `clientSecret` based on the README's PayPal config example.
   - **Validation**: Requires live API testing to confirm credentials work.

3. **Credential mapping for Cybersource**: Assumed `api_key` → `apiKey`, `key1` → `merchantAccount`, `api_secret` → `apiSecret` based on the README and the `bankofamerica` config (which uses the same auth scheme).
   - **Validation**: Requires live API testing to confirm credentials work.

4. **Amount threshold**: Interpreted "amount > 100 USD" as meaning the dollar amount, not minor units. So 100 USD = 10000 minor units is the threshold.
   - **Validated**: Consistent with standard payment processing conventions where amounts in APIs are specified in minor units (cents).

5. **Adyen as third connector**: Assumed Adyen should be included as a connector for sub-$100 USD routing based on the validation requirements, even though it was not listed in the initial connector specification.
   - **Validated**: Adyen credentials are available in creds.json and the SDK supports Adyen.

6. **Test mode**: Assumed all transactions should use `testMode: true` since we're working with sandbox credentials.
   - **Validated**: Credentials in creds.json appear to be sandbox/test credentials based on key prefixes and merchant account names.

---

## Reference

**Local file path**: `/home/grace/session-1-test/hyperswitch-prism/grace-payment-integration/friction-log.md`
