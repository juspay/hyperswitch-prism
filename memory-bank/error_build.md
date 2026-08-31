# Build Errors - Checkout Connector

## Trait Implementation Errors
1. Missing trait implementations for `ConnectorIntegrationV2`
2. Conflicting implementations of traits
3. Private trait access issues for `ConnectorCommon` and `ConnectorIntegrationV2`

## Struct Field Errors
1. Missing fields in `CheckoutPaymentRequest`:
   - source
   - amount
   - currency
   - reference
   - description
   - capture
   - three_ds
   - customer
   - metadata

2. Incorrect field access:
   - No field `description` on type `PaymentsAuthorizeData`
   - No field `return_url` on type `PaymentsAuthorizeData`
   - No field `reference` on type `CheckoutPaymentResponse`

## Type Mismatch Errors
1. Expected `String`, found `CheckoutPaymentStatus` in payment status handling
2. Expected `&Connectors`, found `Connectors` in base_url calls
3. `ResponseId` doesn't implement `std::fmt::Display`

## Method Errors
1. No method `json` found for struct `Response`
2. No method `is_manual` found for enum `Option`

## Next Steps
1. Fix trait implementations and visibility
2. Correct struct field definitions
3. Implement proper type conversions
4. Add missing trait implementations
5. Fix method calls with correct types 

Build Failed (after adding transformers):

**Errors in `transformers.rs`:**
1.  `E0252: the name PaymentsResponseData is defined multiple times` (due to importing from `hyperswitch_domain_models` and `domain_types`).
2.  `E0432: unresolved import common_enums` (should be `hyperswitch_common_enums`).
3.  `E0117: orphan rule violation` for `impl TryFrom<...> for RouterDataV2<...>`. Needs refactoring.
4.  `E0063: missing fields charge_id and mandate_reference in initializer of PaymentsResponseData::TransactionResponse`.

**Errors in `checkout.rs`:**
5.  `E0277: trait bound CheckoutPaymentRequest: TryFrom<&...> is not satisfied` (likely due to `PaymentsResponseData` mismatch in `RouterDataV2` signature).
6.  `E0599: no method named change_context found` (missing `use error_stack::ResultExt;`).

**Warnings:**
*   Multiple unused imports in both files.

**Full Output:**
```
   Compiling connector-integration v0.1.0 (/Users/sweta.sharma/Desktop/Juspay/connector-service/backend/conne\nctor-integration)
error[E0252]: the name `PaymentsResponseData` is defined multiple times
 --> backend/connector-integration/src/connectors/checkout/transformers.rs:8:77
  |
4 | use hyperswitch_domain_models::router_response_types::PaymentsResponseData;
  |     ---------------------------------------------------------------------- previous import of the type `P\naymentsResponseData` here
...
8 | use domain_types::connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData};
  |                                                                             ^^^^^^^^^^^^^^^^^^^^ `Payment\nsResponseData` reimported here
  |
  = note: `PaymentsResponseData` must be defined only once in the type namespace of this module
help: you can use `as` to change the binding name of the import
  |
8 | use domain_types::connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData as Other\nPaymentsResponseData};
  |                                                                                                  ++++++++\n++++++++++++++++++++

error[E0432]: unresolved import `common_enums`
  --> backend/connector-integration/src/connectors/checkout/transformers.rs:14:5
   |
14 | use common_enums::AttemptStatus; // For status mapping
   |     ^^^^^^^^^^^^ use of unresolved module or unlinked crate `common_enums`
   |
   = help: if you wanted to use a crate named `common_enums`, use `cargo add common_enums` to add it to your \n`Cargo.toml`

warning: unused import: `hyperswitch_common_utils::ext_traits::ByteSliceExt`
  --> backend/connector-integration/src/connectors/checkout.rs:16:5
   |
16 | use hyperswitch_common_utils::ext_traits::ByteSliceExt; // For .parse_struct()
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: unused import: `hyperswitch_domain_models::router_data::RouterData`
 --> backend/connector-integration/src/connectors/checkout/transformers.rs:3:5
  |
3 | use hyperswitch_domain_models::router_data::RouterData;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `PaymentsResponseData`
 --> backend/connector-integration/src/connectors/checkout/transformers.rs:8:77
  |
8 | use domain_types::connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData};
  |                                                                             ^^^^^^^^^^^^^^^^^^^^

warning: unused imports: `ResultExt` and `report`
  --> backend/connector-integration/src/connectors/checkout/transformers.rs:11:19
   |
11 | use error_stack::{ResultExt, report};
   |                   ^^^^^^^^^  ^^^^^^

warning: unused import: `hyperswitch_domain_models::router_response_types::RedirectForm`
  --> backend/connector-integration/src/connectors/checkout/transformers.rs:13:5
   |
13 | use hyperswitch_domain_models::router_response_types::RedirectForm;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `CheckoutPaymentsResponse`
  --> backend/connector-integration/src/connectors/checkout.rs:36:50
   |
36 | use self::transformers::{CheckoutPaymentRequest, CheckoutPaymentsResponse};
   |                                                  ^^^^^^^^^^^^^^^^^^^^^^^^

error[E0117]: only traits defined in the current crate can be implemented for types defined outside of the cr\nate
   --> backend/connector-integration/src/connectors/checkout/transformers.rs:109:1
    |
109 | impl TryFrom<(CheckoutPaymentsResponse, &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData\n, PaymentsResponseData>)> for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsRespons\neData...
    | ^^^^^--------------------------------------------------------------------------------------------------\n-------------------------^^^^^-------------------------------------------------------------------------------\n------
    |      |

                              |
    |      |

                              `RouterDataV2` is not defined in the current crate
    |      this is not defined in the current crate because this is a foreign trait
    |
    = note: impl doesn't have any local type before any uncovered type parameters
    = note: for more information see https://doc.rust-lang.org/reference/items/implementations.html#orphan-ru\nles
    = note: define and implement a trait or new type instead

error[E0063]: missing fields `charge_id` and `mandate_reference` in initializer of `hyperswitch_domain_models\n::router_response_types::PaymentsResponseData`
   --> backend/connector-integration/src/connectors/checkout/transformers.rs:121:38
    |
121 |         let payments_response_data = PaymentsResponseData::TransactionResponse {
    |                                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ missing `charge_id` and \n`mandate_reference`

error[E0277]: the trait bound `CheckoutPaymentRequest: TryFrom<&...>` is not satisfied
   --> backend/connector-integration/src/connectors/checkout.rs:171:29
    |
171 |         let connector_req = CheckoutPaymentRequest::try_from(req)
    |                             ^^^^^^^^^^^^^^^^^^^^^^ unsatisfied trait bound
    |
    = help: the trait `TryFrom<&RouterDataV2<_, _, _, domain_types::connector_types::PaymentsResponseData>>` \nis not implemented for `CheckoutPaymentRequest`
            but trait `TryFrom<&RouterDataV2<_, _, _, hyperswitch_domain_models::router_response_types::Payme\nntsResponseData>>` is implemented for it
    = help: for that trait implementation, expected `hyperswitch_domain_models::router_response_types::Paymen\ntsResponseData`, found `domain_types::connector_types::PaymentsResponseData`
    = note: required for `&RouterDataV2<Authorize, PaymentFlowData, ..., ...>` to implement `Into<CheckoutPay\nmentRequest>`
    = note: required for `CheckoutPaymentRequest` to implement `TryFrom<&RouterDataV2<domain_types::connector\n_flow::Authorize, domain_types::connector_types::PaymentFlowData, domain_types::connector_types::PaymentsAuth\norizeData, domain_types::connector_types::PaymentsResponseData>>`
    = note: the full name for the type has been written to \'/Users/sweta.sharma/Desktop/Juspay/connector-serv\nice/target/debug/deps/connector_integration-35821ce041388d3e.long-type-14685648063454630848.txt'
    = note: consider using `--verbose` to print the full type name to the console

error[E0599]: no method named `change_context` found for enum `Result` in the current scope
   --> backend/connector-integration/src/connectors/checkout.rs:172:14
    |
171 |   ...   let connector_req = CheckoutPaymentRequest::try_from(req)
    |  ___________________________-
172 | | ...       .change_context(errors::ConnectorError::RequestEncodingFailed)?; // Propagate errors fro...
    | |___________-^^^^^^^^^^^^^^
    |
   ::: /Users/sweta.sharma/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/error-stack-0.4.1/src/result.\nrs:96:8
    |
96  |       fn change_context<C>(self, context: C) -> core::result::Result<Self::Ok, Report<C>>
    |          -------------- the method is available for `Result<CheckoutPaymentRequest, Infallible>` here
    |
    = help: items from traits can only be used if the trait is in scope
help: trait `ResultExt` which provides `change_context` is implemented but not in scope; perhaps you want to \nimport it
    |
4   + use error_stack::ResultExt;
    |
help: there is a method `change_context_lazy` with a similar name
    |
172 |             .change_context_lazy(errors::ConnectorError::RequestEncodingFailed)?; // Propagate errors f\nrom TryFrom
    |                            +++++

error[E0277]: the trait bound `CheckoutPaymentRequest: From<&...>` is not satisfied
   --> backend/connector-integration/src/connectors/checkout.rs:171:29
    |
171 |         let connector_req = CheckoutPaymentRequest::try_from(req)
    |                             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ unsatisfied trait bound
    |
    = help: the trait `TryFrom<&RouterDataV2<_, _, _, domain_types::connector_types::PaymentsResponseData>>` \nis not implemented for `CheckoutPaymentRequest`
            but trait `TryFrom<&RouterDataV2<_, _, _, hyperswitch_domain_models::router_response_types::Payme\nntsResponseData>>` is implemented for it
    = help: for that trait implementation, expected `hyperswitch_domain_models::router_response_types::Paymen\ntsResponseData`, found `domain_types::connector_types::PaymentsResponseData`
    = note: required for `&RouterDataV2<Authorize, PaymentFlowData, ..., ...>` to implement `Into<CheckoutPay\nmentRequest>`
    = note: required for `CheckoutPaymentRequest` to implement `TryFrom<&RouterDataV2<domain_types::connector\n_flow::Authorize, domain_types::connector_types::PaymentFlowData, domain_types::connector_types::PaymentsAuth\norizeData, domain_types::connector_types::PaymentsResponseData>>`
    = note: the full name for the type has been written to \'/Users/sweta.sharma/Desktop/Juspay/connector-serv\nice/target/debug/deps/connector_integration-35821ce041388d3e.long-type-13605348631184523000.txt'
    = note: consider using `--verbose` to print the full type name to the console

error[E0308]: mismatched types
   --> backend/connector-integration/src/connectors/checkout/transformers.rs:102:27
    |
102 |         redirection_data: None, 
    |                           ^^^^ expected `Box<Option<RedirectForm>>`, found `Option<_>`
    |
    = note: expected struct `Box<std::option::Option<RedirectForm>>`
                 found enum `std::option::Option<_>`
    = note: for more on the distinction between the stack and the heap, read https://doc.rust-lang.org/book/c\nh15-01-box.html, https://doc.rust-lang.org/rust-by-example/std/box.html, and https://doc.rust-lang.org/std/bo\nxed/index.html
help: store this in the heap by calling `Box::new`
    |
102 |         redirection_data: Box::new(None), 
    |                           +++++++++    +

For more information about this error, try `rustc --explain E0308`.
warning: `connector-integration` (lib) generated 2 warnings
error: could not compile `connector-integration` (lib) due to 7 previous errors; 2 warnings emitted 