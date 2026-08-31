Cargo Test Failed:

**Errors in `checkout.rs` tests:**
1.  `E0433: unresolved import bytes`.
2.  `E0277: trait bound CardNumber: From<String> is not satisfied`.
3.  `E0308: mismatched types` for `MerchantId::from("test_merchant")`.
4.  `E0599: no method named is_object found` for serialized request body.
5.  `E0369: binary operation == cannot be applied to type ResponseId` (missing `PartialEq`).

**Errors in `adyen/test.rs`, `razorpay/test.rs`:**
*   `E0063: missing field checkout in initializer of domain_types::types::Connectors` (multiple occurrences).

**Warnings:**
*   Multiple unused imports/fields.

**Full Output:**
```
<PASTED FULL CARGO TEST OUTPUT HERE - Omitted for brevity in this summary> 