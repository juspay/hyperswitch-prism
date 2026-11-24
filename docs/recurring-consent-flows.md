# Recurring consent connector flows

Prism exposes individual connector operations. Hyperswitch owns sequencing and later webhook/status reconciliation. Consent creation or approval does not settle a payment.

| Operation | Domain marker | Contract |
| --- | --- | --- |
| `RecurringPaymentService.RequestConfirmation` | `PushNotification` | Mandate ID, payer account, RFC 3339 expiry; optional caller request ID, connector state, test mode, feature data. |
| `RecurringPaymentService.GetQr` | `GenerateQr` | Mandate ID, optional initial Pix transaction ID; optional caller request ID, connector state, test mode, feature data. |

Both operations are amountless. Their shared response preserves the mandate reference, connector confirmation ID where applicable, redirect/QR data, connector state, feature data, response headers and request/response diagnostics. A connector error carries the structured error details used by other Prism operations.

The push sequence is `SetupRecurring -> RequestConfirmation`. The QR sequence is `CreateServerSessionAuthenticationToken -> SetupRecurring -> GetQr`: pass the session token (QR location ID) to setup and pass the resulting mandate ID to the QR operation. An initial transaction reference is accepted; creating that initial payment is outside these flows. Session request data preserves access-token and connector-customer state for an authenticated session operation.

The consent flows accept `Pending`, `AuthenticationPending`, `Authorized`, `AuthenticationFailed`, `Failure`, `Voided`, and `Unresolved`. `Authorized` describes consent approval. Cash-payment statuses such as `Charged` are rejected on success and error conversion. Requests begin in `Pending`; connector error attempt statuses are checked against the same flow rules.

Typed Pix Automático setup and charge configuration accompanies the explicit push/QR bank-transfer instruments. Configuration paired with an unrelated instrument is rejected, as is a recurring Pix instrument without its configuration. Recurring charges select the push/QR method explicitly through `payment_method_type`; any provided instrument must agree with its Pix-versus-non-Pix classification. Existing card customer-acceptance requirements are preserved. Agreement setup and recurring charging remain separate operations.

The operations are wired through gRPC, HTTP (`/recurring_payments/request_confirmation` and `/recurring_payments/qr`), FFI transformers/registration, and generated Rust, Python, JavaScript and Kotlin clients. The gRPC FFI bridge dispatches both methods. Every existing connector explicitly reports these flows as unsupported until it opts in; there are no vendor HTTP implementations in this change.

## Validation

The contract tests use isolated services and local endpoints. They do not require vendor credentials or establish vendor homologation.

```sh
cargo test --locked -p grpc-server --test recurring_consent_flows_test
cargo test --locked -p domain_types --lib
cargo test --locked -p domain_types --test recurring_consent
cargo build --locked -p connector-integration -p grpc-server -p ffi -p hyperswitch-grpc-ffi -p hyperswitch-payments-client -p rust-grpc-client
cargo run --locked --bin check_connector_specs
cargo fmt --all --check
python scripts/generators/code/generate.py
```

Validation recorded on 24 November 2026. Rust commands ran with cached dependencies (`--offline`), `CARGO_BUILD_JOBS=2`, and `CARGO_PROFILE_DEV_DEBUG=0`:

| Check | Result |
| --- | --- |
| Domain tests | 11 tests passed, including consent conversion, state preservation, setup acceptance and instrument/configuration checks. |
| gRPC FFI bridge dispatch | 1 test passed: both method names reach typed protobuf decoding. |
| Affected Rust crates and SDKs | Build passed for `connector-integration`, `grpc-server`, `ffi`, `hyperswitch-grpc-ffi`, `hyperswitch-payments-client`, and `rust-grpc-client`. |
| Connector-spec structural checker | Passed: 123 connector/spec directories match, all 123 connectors' flows are accounted for, and no suite or webhook-fixture coverage is missing. |
| Generic gRPC/server compilation | Passed while compiling the contract test target. |
| gRPC contract test execution | Unverified: the sandbox denied the isolated localhost listener (`PermissionDenied`) before any RPC assertions ran. The request to execute outside that restriction was interrupted before approval. |
| Formatting and diff whitespace | Passed. |
| Existing protobuf compatibility | Passed comparison across 718 existing messages and 89 enums. |
| Client code generation | Reproducible across 14 generated artifacts. |
| Python and generated JavaScript syntax | Passed Python AST parsing (generated clients, type stub, and generator) and `node --check` for the generated JavaScript flow file. |
| Java/Kotlin and TypeScript client compilation | Unverified. |

Code generation and bridge dispatch checks do not establish that every SDK compiles or that the RPCs execute successfully. The structural checker's existing `EventService/NotifyConnector` gap is informational (29 of 30 suites are testable). Rust linking emitted a macOS warning about a large unwind table; the builds completed successfully.

## Source guide

- Public contracts: `crates/types-traits/grpc-api-types/proto/{services,payment,payment_methods}.proto`.
- Domain adapters and response/status validation: `crates/types-traits/domain_types/src/pix_automatico.rs`.
- Generic connector defaults: `crates/integrations/connector-integration/src/default_implementations.rs`.
- Runtime dispatch: `crates/grpc-server/grpc-server/src/server/payments.rs` and `sdk/grpc-ffi/src/lib.rs`.
- Contract tests: `crates/grpc-server/grpc-server/tests/recurring_consent_flows_test.rs`.
- Domain regression tests: `crates/types-traits/domain_types/tests/recurring_consent.rs`.

This change provides generic connector-flow infrastructure. Connector-specific implementations are outside its scope.
