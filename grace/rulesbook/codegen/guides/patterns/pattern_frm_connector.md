# FRM Connector Pattern

## Overview

An **FRM connector** (Fraud & Risk Management) scores a transaction for fraud
risk and receives later lifecycle notifications. FRM now has its own connector
directory:

```text
crates/integrations/connector-integration/src/frm_connectors/
crates/integrations/connector-integration/src/frm_connectors.rs
```

Do **not** add a new FRM connector under `src/connectors/`, do not add it to
`ConnectorEnum` / `ConnectorData`, and do not create `connector_specs/<name>/`.
The certification checker scans only `src/connectors/`; a specs directory for a
connector that lives under `src/frm_connectors/` is a CI failure, not a fix.

At HEAD the live exemplars are:

| Connector | File | Real FRM flows | Stubbed / support flows |
| --- | --- | --- | --- |
| Kount | `frm_connectors/kount.rs` | `PreRiskCheck`, `FrmPaymentOutcome`, `FrmRefundProcessed` | real `ServerAuthenticationToken` and local `PreAuthenticate`; `PostRiskCheck`, `FrmChargebackReceived` stubbed with `frm_flow_not_implemented!` |
| nSure | `frm_connectors/nsure.rs` | `PreRiskCheck`, `FrmPaymentOutcome`, `FrmRefundProcessed`, `FrmChargebackReceived` | `ServerAuthenticationToken`, `PreAuthenticate`, `PostRiskCheck` stubbed |

## Key Components

- **Directory**: `crates/integrations/connector-integration/src/frm_connectors/`
- **Module file**: `crates/integrations/connector-integration/src/frm_connectors.rs`
- **Service trait**: `interfaces::connector_types::FrmServiceTrait`
- **Boxed form**: `BoxedFrmConnector = Box<&'static (dyn FrmServiceTrait + Sync)>`
- **Provider**: `connector_integration::types::FrmConnectorData`
- **Category enum**: `domain_types::connector_types::FrmConnectorEnum`
- **Flow data**: `domain_types::frm::frm_types::FrmFlowData`
- **Flow markers**: `PreRiskCheck`, `PostRiskCheck`, `FrmPaymentOutcome`, `FrmRefundProcessed`, `FrmChargebackReceived`
- **Routing header**: `x-frm-connector` (`common_utils::consts::X_FRM_CONNECTOR_NAME`)

## Architecture

```text
FraudAndRiskManagementService::{PreRiskCheck, PostRiskCheck}
CompositeFraudAndRiskManagementService::{PreRiskCheck, PostRiskCheck}
EventService::NotifyConnector
    |
    v
ConnectorVariant::Frm(FrmConnectorEnum::Foo)
    |
    v
FrmConnectorData::convert_connector
    |
    v
frm_connectors::Foo::<DefaultPCIHolder>::new()
    |
    v
RouterDataV2<FrmFlow, FrmFlowData, Frm*Request, Frm*Response>
```

The router must resolve FRM requests to `ConnectorVariant::Frm(..)`. Add the
connector's `AuthType` arm in `impl ForeignTryFrom<AuthType> for ConnectorVariant`
as:

```rust
AuthType::Foo(_) => Ok(Self::Frm(FrmConnectorEnum::Foo)),
```

Using `ConnectorVariant::Payment(ConnectorEnum::Foo)` is the old rule and is now
wrong for FRM-first connectors.

### Routing, Flow Data, And gRPC Surface

Every FRM request must carry `x-frm-connector`, whose constant is
`common_utils::consts::X_FRM_CONNECTOR_NAME`. The
`connector_variant_from_config_and_metadata` branch in
`crates/types-traits/ucs_interface_common/src/auth.rs` checks that header and
selects `FrmConnectorEnum::foreign_try_from(config)`. Without it, the generic
fallback resolves the config through the payment registry, so
`FrmConnectorData::from_connector_variant` cannot reach the FRM connector.

The composite FRM layer fetches an OAuth token before risk checks when the
connector requires one. `build_access_token_request` in
`crates/internal/composite-service/src/frm.rs` supplies the token, and the
composite layer stores it in `FrmFlowData.access_token`.

```rust
pub struct FrmFlowData {
    pub merchant_id: MerchantId,
    pub connectors: Arc<Connectors>,
    pub access_token: Option<ServerAuthenticationTokenResponseData>,
    pub raw_connector_response: Option<Secret<String>>,
    pub typed_connector_response: Option<String>,
    pub raw_connector_request: Option<Secret<String>>,
    pub typed_connector_request: Option<String>,
    pub connector_response_headers: Option<http::HeaderMap>,
}
```

The seven non-token fields are plumbing for request/response capture; read the
bearer token from `access_token`, as Kount's `frm_bearer_header` does.

The two direct RPCs are `FraudAndRiskManagementService/{PreRiskCheck,PostRiskCheck}`
and their composite equivalents. `FrmPaymentOutcome`, `FrmRefundProcessed`, and
`FrmChargebackReceived` arrive through `EventService/NotifyConnector`, not as
separate FRM RPCs. In `grpc-server/src/server/events.rs`,
`FRM_PAYMENT_SUCCEEDED` and `FRM_PAYMENT_FAILURE` intentionally collapse into
one `FrmPaymentOutcome` dispatch arm; the distinction is carried by
`FrmPaymentOutcomeRequest.payment_status` / `frm_decision`.

### Request And Response Data Shapes

All five FRM flows use `FrmFlowData`. `PreRiskCheckResponse` and
`PostRiskCheckResponse` carry `frm_decision`, `risk_score`, `reason`,
`frm_transaction_id`, and `status_code`; notification responses carry only
`status_code`. Read the complete request field lists from
`crates/types-traits/domain_types/src/frm/frm_types.rs` rather than copying a
stale doc: pre-risk has 13 fields, post-risk 12, payment outcome 8, refund
processed 8, and chargeback received 7.

Use `common_enums::FrmDecision` in connector transformers. It is distinct from
`grpc_api_types::frm::FrmDecision` in `payment.proto`; the conversion lives in
`domain_types/src/frm/types.rs`, and proto `Unspecified` folds onto domain
`Review`. Map connector decisions exhaustively and handle unknown wire values
explicitly; never let an enum mismatch silently become `Approve`.

`PreRiskCheckResponse.frm_transaction_id` is the join key. Return it when the
provider creates a risk transaction because the payment-outcome, refund, and
chargeback notifications send that value back later.

## Service Trait Requirements

`FrmServiceTrait` has this supertrait list:

```rust
pub trait FrmServiceTrait:
    ConnectorCommon
    + ValidationTrait
    + ServerAuthentication
    + PreRiskCheckV2
    + PostRiskCheckV2
    + FrmPaymentOutcomeV2
    + FrmRefundProcessedV2
    + FrmChargebackReceivedV2
    + PaymentPreAuthenticateV2<domain_types::payment_method_data::DefaultPCIHolder>
{
}
```

Two requirements are easy to miss:

- `ServerAuthentication` is required even when the connector has no token
  endpoint. If the connector has no token flow, satisfy it with
  `macro_connector_flow_status_impls!(not_implemented: [ServerAuthenticationToken, ...])`.
- `PaymentPreAuthenticateV2<DefaultPCIHolder>` is fixed to `DefaultPCIHolder`.
  The aggregate `FrmServiceTrait` impl must be monomorphized:

```rust
impl connector_types::FrmServiceTrait
    for Foo<domain_types::payment_method_data::DefaultPCIHolder>
{
}
```

Do not write `impl<T> FrmServiceTrait for Foo<T>`; it fails because
`PaymentPreAuthenticateV2<DefaultPCIHolder>` is not satisfied for arbitrary `T`.

## Registration Sites

Add a new FRM connector to these places:

| # | Site | File | What to add |
| --- | --- | --- | --- |
| 1 | FRM module | `crates/integrations/connector-integration/src/frm_connectors.rs` | `pub mod foo;` and `pub use self::foo::Foo;` |
| 2 | FRM source | `crates/integrations/connector-integration/src/frm_connectors/foo.rs` and `foo/transformers.rs` | `ConnectorCommon`, `ValidationTrait`, required support-flow stubs, FRM flow impls, monomorphized `FrmServiceTrait` |
| 3 | FRM enum | `crates/types-traits/domain_types/src/connector_types.rs` | `Foo` in `pub enum FrmConnectorEnum` |
| 4 | FRM auth conversion | same file, `impl ForeignTryFrom<AuthType> for FrmConnectorEnum` | `AuthType::Foo(_) => Ok(Self::Foo)` |
| 5 | Router variant | same file, `impl ForeignTryFrom<AuthType> for ConnectorVariant` | `AuthType::Foo(_) => Ok(Self::Frm(FrmConnectorEnum::Foo))` |
| 6 | FRM provider | `crates/integrations/connector-integration/src/types.rs` | `FrmConnectorEnum::Foo => Box::new(frm_connectors::Foo::<DefaultPCIHolder>::new())` |
| 7 | URL patcher | `crates/types-traits/domain_types/src/types.rs`, `patch_frm_connector_urls` | exhaustive `FrmConnectorEnum::Foo => patched.foo.apply(params_patch),` |
| 8 | `Connectors` struct | same file | `pub foo: ConnectorParams,` |
| 9 | Config TOMLs | `config/development.toml`, `config/sandbox.toml`, `config/production.toml` | `foo.base_url = "..."` and any extra URL fields the connector needs |
| 10 | Connector config enum | `crates/types-traits/domain_types/src/router_data.rs` | `ConnectorSpecificConfig::Foo { ... }` plus auth conversions |
| 11 | Proto config | `crates/types-traits/grpc-api-types/proto/payment.proto` | `FooConfig` and a `ConnectorSpecificConfig` oneof entry |
| 12 | Proto connector enum | same proto file | `FOO = <next ordinal>;` if the connector config must be addressable by the shared proto enum |
| 13 | Superposition | `config/superposition.toml` | only when the connector needs dynamic URL overrides; Kount has this, nSure does not |

Do **not** add these for an FRM-first connector:

- `crates/integrations/connector-integration/src/connectors/foo.rs`
- `crates/integrations/connector-integration/src/connectors.rs`
- `ConnectorEnum::Foo`
- `ConnectorData::convert_connector`
- `default_implementations.rs`
- `field-probe` dummy auth
- `connector_specs/foo/specs.json`

## Flow Bindings

All five FRM flows use `FrmFlowData`.

| Marker | Marker trait | Request | Response | Reached by |
| --- | --- | --- | --- | --- |
| `PreRiskCheck` | `PreRiskCheckV2` | `PreRiskCheckRequest` | `PreRiskCheckResponse` | `FraudAndRiskManagementService/PreRiskCheck` |
| `PostRiskCheck` | `PostRiskCheckV2` | `PostRiskCheckRequest` | `PostRiskCheckResponse` | `FraudAndRiskManagementService/PostRiskCheck` |
| `FrmPaymentOutcome` | `FrmPaymentOutcomeV2` | `FrmPaymentOutcomeRequest` | `FrmPaymentOutcomeResponse` | `EventService/NotifyConnector` with `FRM_PAYMENT_SUCCEEDED` or `FRM_PAYMENT_FAILURE` |
| `FrmRefundProcessed` | `FrmRefundProcessedV2` | `FrmRefundProcessedRequest` | `FrmRefundProcessedResponse` | `EventService/NotifyConnector` with `FRM_REFUND_PROCESSED` |
| `FrmChargebackReceived` | `FrmChargebackReceivedV2` | `FrmChargebackReceivedRequest` | `FrmChargebackReceivedResponse` | `EventService/NotifyConnector` with `FRM_CHARGEBACK_RECEIVED` |

The support-flow bindings required by `FrmServiceTrait` are not FRM data:

| Marker | Trait | Resource data | Request | Response |
| --- | --- | --- | --- | --- |
| `ServerAuthenticationToken` | `ServerAuthentication` | `MerchantAuthenticationFlowData` | `ServerAuthenticationTokenRequestData` | `ServerAuthenticationTokenResponseData` |
| `PreAuthenticate` | `PaymentPreAuthenticateV2<DefaultPCIHolder>` | `PaymentFlowData` | `PaymentsPreAuthenticateData<DefaultPCIHolder>` | `PaymentsResponseData` |

## Macro Rules

Use the regular connector macros from `crate::connectors::macros` inside
`frm_connectors/*`.

| Macro | Use |
| --- | --- |
| `create_amount_converter_wrapper!` | amount conversion wrapper |
| `create_all_prerequisites!` | declares each real flow and generates the generic connector struct |
| `macro_connector_implementation!` | one real outbound FRM or token flow |
| `macro_connector_local_flow_implementation!` | local-only flow such as Kount DDC `PreAuthenticate` |
| `macro_connector_flow_status_impls!` | support-flow stubs such as `ServerAuthenticationToken` / `PreAuthenticate`; this macro emits the marker trait and the stub |
| `frm_flow_not_implemented!` | FRM-flow stubs only; this macro does **not** emit the marker trait |

For every `frm_flow_not_implemented!` call, also add the bare marker impl:

```rust
macros::frm_flow_not_implemented!(
    connector: Foo,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    flow: PostRiskCheck,
    request: PostRiskCheckRequest,
    response: PostRiskCheckResponse,
    flow_name: "post_risk_check",
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PostRiskCheckV2 for Foo<T>
{
}
```

For a connector with no token flow and no DDC flow, copy nSure's shape:

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PreRiskCheckV2 for Foo<T>
{
}
// ...the other four FRM marker traits...

macros::macro_connector_flow_status_impls!(
    connector: Foo,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [ServerAuthenticationToken, PreAuthenticate],
);

impl connector_types::FrmServiceTrait
    for Foo<domain_types::payment_method_data::DefaultPCIHolder>
{
}
```

## Data Rules

- Use `common_enums::FrmDecision` in transformers, not the proto enum.
- Map connector decisions exhaustively. Put `#[serde(other)] Unknown` on the
  wire enum and map `Unknown` explicitly to `FrmDecision::Error` or
  `FrmDecision::Review`, never silently to `Approve`.
- Return `frm_transaction_id` from `PreRiskCheck`; lifecycle notify flows use it
  to address the original risk transaction.
- Do not turn FRM transport/config failures into fraud rejections. Use
  `FrmDecision::Error` when the FRM check itself failed.
- Notify responses carry only `status_code`; preserve useful response bodies on
  the event builder / raw typed connector fields before discarding them.

## Device Data Collection

Kount is the current DDC exemplar. Its `PreAuthenticate` is local-only and emits
a browser script through `macro_connector_local_flow_implementation!`. If a new
FRM connector has browser-side DDC:

1. Implement `PaymentPreAuthenticateV2<T>` for the connector.
2. Use `resource_common_data: PaymentFlowData`.
3. Override `ValidationTrait::next_authentication_step` so the standalone
   authentication loop actually dispatches `PreAuthenticate`.
4. Escape all values interpolated into `<script>` output; Kount's
   `js_string_escape` is the local reference.

## Certification And Tests

- Do not create `connector_specs/<name>/` for a pure FRM connector. The checker
  reads exactly `src/connectors/`; specs without a matching payment connector
  file fail Phase 1.
- The five FRM markers remain in `OUT_OF_SCOPE_FLOWS`, which means "no suite
  exists yet", not "do not implement".
- Add focused unit tests for transformer logic: auth extraction, decision
  mapping including `Unknown`, transaction-id fallback, amount conversion, and
  any script escaping.
- Manual FRM scenarios to record in the PR: pre-risk `APPROVE` / `REJECT` /
  `REVIEW`, FRM service down or bad credentials expecting `ERROR`, payment
  success/failure notify, refund notify, and chargeback notify when supported.

## Common Gotchas

1. **Creating `connectors/foo.rs` for FRM.**
   - Use `frm_connectors/foo.rs`; FRM is a first-class sibling directory now.

2. **Adding a connector specs directory.**
   - Do not. `check_connector_specs.rs` only scans `src/connectors/`, so a specs
     directory for `src/frm_connectors/foo.rs` has no matching integration file.

3. **Mapping `AuthType::Foo` to Payment.**
   - FRM-first connectors map to `ConnectorVariant::Frm(FrmConnectorEnum::Foo)`.

4. **Assuming `frm_flow_not_implemented!` emits marker traits.**
   - It emits only the `ConnectorIntegrationV2` stub. Add `PreRiskCheckV2`,
     `PostRiskCheckV2`, `FrmPaymentOutcomeV2`, `FrmRefundProcessedV2`, or
     `FrmChargebackReceivedV2` marker impls yourself.

5. **Expecting five FRM RPCs.**
   - There are two FRM service RPCs. Payment outcome, refund processed, and
     chargeback received arrive through `EventService/NotifyConnector`.

## Cross-References

- Parent index: [./README.md](./README.md)
- Kount exemplar: `crates/integrations/connector-integration/src/frm_connectors/kount.rs`
- nSure exemplar: `crates/integrations/connector-integration/src/frm_connectors/nsure.rs`
- Macros: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Auth dispatch: [pattern_authentication_dispatch.md](./pattern_authentication_dispatch.md)
- Server token flow: [pattern_server_authentication_token.md](./pattern_server_authentication_token.md)
