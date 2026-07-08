# PayoutTransfer Flow Pattern

## Overview

The `PayoutTransfer` flow moves funds from the processor to the payout beneficiary using a previously-established identifier (or inline payout-method data). It is driven by the `PayoutService::transfer` gRPC handler (`crates/grpc-server/grpc-server/src/server/payouts.rs:64-82`) and dispatched through `internal_payout_transfer` (`crates/grpc-server/grpc-server/src/server/payouts.rs:279-292`) under the `FlowName::PayoutTransfer` marker (`crates/types-traits/domain_types/src/connector_flow.rs:126`).

At the pinned SHA this is the **only payout flow with a concrete, non-default connector implementation**: `itaubank` performs a real HTTP `POST` to Itaú's Brazilian PIX/transfers endpoint at `crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424`. This pattern documents that implementation as the canonical reference.

### Key Components

- **Flow marker**: `domain_types::connector_flow::PayoutTransfer` — `crates/types-traits/domain_types/src/connector_flow.rs:77`.
- **Flow data**: `domain_types::payouts::payouts_types::PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:13`.
- **Request data**: `domain_types::payouts::payouts_types::PayoutTransferRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:100-111`.
- **Response data**: `domain_types::payouts::payouts_types::PayoutTransferResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:114-119`.
- **Marker trait**: `interfaces::connector_types::PayoutTransferV2` — `crates/types-traits/interfaces/src/connector_types.rs:657-665`.
- **Primary connector file**: `crates/integrations/connector-integration/src/connectors/itaubank.rs`.
- **Primary transformers file**: `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs`.

## Table of Contents

1. [Overview](#overview)
2. [Architecture Overview](#architecture-overview)
3. [Connectors with Full Implementation](#connectors-with-full-implementation)
4. [Common Implementation Patterns](#common-implementation-patterns)
5. [Connector-Specific Patterns](#connector-specific-patterns)
6. [Code Examples](#code-examples)
7. [Integration Guidelines](#integration-guidelines)
8. [Best Practices](#best-practices)
9. [Common Errors / Gotchas](#common-errors--gotchas)
10. [Testing Notes](#testing-notes)
11. [Cross-References](#cross-references)

## Architecture Overview

```
PayoutService::transfer (gRPC)
    │   crates/grpc-server/grpc-server/src/server/payouts.rs:64
    ▼
internal_payout_transfer
    │   crates/grpc-server/grpc-server/src/server/payouts.rs:279-292
    ▼
ServerAuthenticationToken (if should_do_access_token == true)
    │   crates/integrations/connector-integration/src/connectors/itaubank.rs:144-146
    ▼
RouterDataV2<PayoutTransfer, PayoutFlowData,
             PayoutTransferRequest, PayoutTransferResponse>
    │
    ├─▶ ConnectorIntegrationV2<PayoutTransfer, ...>::get_url/headers/body
    │       crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424
    │
    ├─▶ transport (HTTP POST /v1/transferencias)
    │
    └─▶ ConnectorIntegrationV2::handle_response_v2
            -> PayoutTransferResponse (payout_status, connector_payout_id, status_code)
```

The generic router-data template (per `PATTERN_AUTHORING_SPEC.md` §7):

```rust
RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
// from crates/types-traits/domain_types/src/router_data_v2.rs:5-19
```

### Flow Type

`domain_types::connector_flow::PayoutTransfer` — unit marker struct at `crates/types-traits/domain_types/src/connector_flow.rs:76-77`. It is parametric on `RouterDataV2`'s first slot via `PhantomData<PayoutTransfer>` (`crates/types-traits/domain_types/src/router_data_v2.rs:7`).

### Request Type

`domain_types::payouts::payouts_types::PayoutTransferRequest` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:99-111`. Shape:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:99-111
#[derive(Debug, Clone)]
pub struct PayoutTransferRequest {
    pub merchant_payout_id: Option<String>,
    pub connector_quote_id: Option<String>,
    pub connector_payout_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub destination_currency: common_enums::Currency,
    pub priority: Option<common_enums::PayoutPriority>,
    pub connector_payout_method_id: Option<String>,
    pub webhook_url: Option<String>,
    pub payout_method_data: Option<PayoutMethodData>,
}
```

Note: `PayoutTransferRequest` is structurally identical to `PayoutCreateRequest` at this SHA; the distinction is encoded purely in the flow marker.

### Response Type

`domain_types::payouts::payouts_types::PayoutTransferResponse` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:113-119`. Shape:

```rust
// From crates/types-traits/domain_types/src/payouts/payouts_types.rs:113-119
#[derive(Debug, Clone)]
pub struct PayoutTransferResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
```

`common_enums::PayoutStatus` is defined at `crates/common/common_enums/src/enums.rs:1134-1149`.

### Resource Common Data

`domain_types::payouts::payouts_types::PayoutFlowData` — `crates/types-traits/domain_types/src/payouts/payouts_types.rs:12-23`. Same struct used by every payout flow at this SHA. Note in particular:

- `access_token: Option<ServerAuthenticationTokenResponseData>` — populated by the prior `ServerAuthenticationToken` flow; unwrapped via `PayoutFlowData::get_access_token` (`crates/types-traits/domain_types/src/payouts/payouts_types.rs:54-59`).
- `test_mode: Option<bool>` — consumed by `build_env_specific_endpoint` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:426-432`).
- `connectors: Connectors` — the base-URL bag, used via `ConnectorCommon::base_url` at `crates/integrations/connector-integration/src/connectors/itaubank.rs:84-86`.

## Connectors with Full Implementation

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --------- | ----------- | ------------ | ----------- | ------------------ | ----- |
| itaubank | `POST` | `application/json` | `{env_base}/v1/transferencias` (env suffix via `build_env_specific_endpoint`) | `ItaubankTransferRequest` — flow-local struct, not reused for any other flow | Full impl at `crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424`; `ItaubankTransferRequest` declared at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:104-113`; requires prior `ServerAuthenticationToken` flow (`crates/integrations/connector-integration/src/connectors/itaubank.rs:144-146`). |

### Current implementation coverage

`itaubank` is the **only** connector with a non-default `ConnectorIntegrationV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>` impl at this SHA. Confirmed by grepping for `PayoutTransfer` across `crates/integrations/connector-integration/src/connectors/`: every match resolves to either (a) the itaubank files listed above, or (b) the `macros.rs` expansion arm at `crates/integrations/connector-integration/src/connectors/macros.rs:1356-1371`. No other connector calls the macro with `PayoutTransfer` in its `payout_flows: [...]` list.

### Stub Implementations

_(none at this SHA for `PayoutTransfer` specifically — itaubank has a full impl; no other connector opts into the stub.)_

## Common Implementation Patterns

### Pattern A — Manual `ConnectorIntegrationV2` impl (used by itaubank)

1. Call `macros::create_all_prerequisites!` with `api: []` and empty `member_functions` if no shared helpers are needed — see `crates/integrations/connector-integration/src/connectors/itaubank.rs:45-51`.
2. Call `macros::macro_connector_payout_implementation!` listing only the flows that should get default stubs, **excluding** `PayoutTransfer` — see `crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66`. itaubank explicitly omits `PayoutTransfer` from that list so the manual impl below does not collide.
3. Implement the marker trait with an empty block — `impl PayoutTransferV2 for Connector<T> {}` at `crates/integrations/connector-integration/src/connectors/itaubank.rs:289-292`.
4. Implement `ConnectorIntegrationV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>` with all six methods — `crates/integrations/connector-integration/src/connectors/itaubank.rs:294-424`.
5. Write `TryFrom<&RouterDataV2<PayoutTransfer, ...>>` for the connector-local request struct in `transformers.rs` — `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:143-223`.

### Pattern B — Macro-only stub

Connectors that need `PayoutTransferV2` to satisfy a bound but have no real backend can list `PayoutTransfer` in the payout macro's `payout_flows: [...]` array. The macro expands at `crates/integrations/connector-integration/src/connectors/macros.rs:1356-1371` into an empty `ConnectorIntegrationV2` impl; all methods fall through to defaults. No connector exercises this path for `PayoutTransfer` at this SHA.

## Connector-Specific Patterns

### itaubank

- **URL construction is env-sensitive.** The base URL is suffixed by `build_env_specific_endpoint` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:426-432`) according to `PayoutFlowData.test_mode`; the final path is always `{prefix}/v1/transferencias` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:318-324`).
- **Access-token dependency is mandatory.** `ValidationTrait::should_do_access_token` returns `true` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:141-147`). `get_headers` fails with `IntegrationError::FailedToObtainAuthType` if `get_access_token` returns `Err` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:335-339`).
- **Amount is serialized as `StringMajorUnit`.** Converter `StringMajorUnitForConnector` is used in the request `TryFrom` (`crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:163-168`).
- **Request body is Portuguese-named.** The outgoing struct `ItaubankTransferRequest` carries fields `valor_pagamento`, `data_pagamento`, `chave`, `recebedor`, etc. (`crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:104-113`). Beneficiary details are nested under `ItaubankRecebedor` (`crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:133-141`).
- **Only `PayoutMethodData::Bank(Bank::Pix(...))` is supported inline.** Other variants cause the `recebedor` field to be `None` (`crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:175-211`), which the API may reject at the processor.
- **Person type is inferred from tax-ID length.** 11-digit IDs are mapped to `ItaubankPersonType::Individual`; others to `Company` (`crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:183-189`).
- **Response parsing is tolerant of dual field names.** `ItaubankTransferResponse` uses `serde(alias = ...)` to accept both `id`/`cod_pagamento` and `status`/`status_pagamento` (`crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:250-256`).

## Code Examples

### Example 1: itaubank trait wiring (marker + integration impl header)

```rust
// From crates/integrations/connector-integration/src/connectors/itaubank.rs:289-324
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PayoutTransferV2 for Itaubank<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for Itaubank<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        let base_url = build_env_specific_endpoint(
            self.base_url(&req.resource_common_data.connectors),
            req.resource_common_data.test_mode,
        );
        Ok(format!("{base_url}/v1/transferencias"))
    }
```

### Example 2: Access-token header construction

```rust
// From crates/integrations/connector-integration/src/connectors/itaubank.rs:326-356
fn get_headers(
    &self,
    req: &RouterDataV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    >,
) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
    let access_token = req.resource_common_data.get_access_token().map_err(|_| {
        errors::IntegrationError::FailedToObtainAuthType {
            context: Default::default(),
        }
    })?;

    Ok(vec![
        (
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        ),
        (headers::ACCEPT.to_string(), "*/*".to_string().into()),
        (
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {access_token}").into_masked(),
        ),
        (
            headers::USER_AGENT.to_string(),
            "Hyperswitch".to_string().into(),
        ),
    ])
}
```

### Example 3: Request `TryFrom` with amount conversion and PIX beneficiary assembly

```rust
// From crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:143-223
impl
    TryFrom<
        &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    > for ItaubankTransferRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let converter = StringMajorUnitForConnector;
        let valor_pagamento = converter
            .convert(req.request.amount, req.request.source_currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let data_pagamento = common_utils::date_time::date_as_yyyymmddthhmmssmmmz()
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let recebedor = match req.request.payout_method_data.clone() {
            Some(PayoutMethodData::Bank(Bank::Pix(PixBankTransfer {
                tax_id,
                bank_branch,
                bank_account_number,
                bank_name,
                ..
            }))) => {
                let tipo_pessoa = tax_id.clone().expose_option().map(|id| {
                    if id.len() == 11 {
                        ItaubankPersonType::Individual
                    } else {
                        ItaubankPersonType::Company
                    }
                });
                // ... agencia parsing, recebedor assembly
                Some(ItaubankRecebedor { /* fields */ })
            }
            _ => None,
        };

        Ok(Self {
            valor_pagamento,
            data_pagamento,
            chave: req.request.connector_payout_id.clone().map(Secret::new),
            referencia_empresa: req.request.merchant_payout_id.clone(),
            identificacao_comprovante: req.request.merchant_payout_id.clone().map(Secret::new),
            informacoes_entre_usuarios: Some(Secret::new("Payout".to_string())),
            recebedor,
        })
    }
}
```

### Example 4: Response handling and status mapping

```rust
// From crates/integrations/connector-integration/src/connectors/itaubank.rs:371-415
fn handle_response_v2(
    &self,
    data: &RouterDataV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    >,
    event_builder: Option<&mut events::Event>,
    res: Response,
) -> CustomResult<
    RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
    errors::ConnectorResponseTransformationError,
> {
    let response: Result<ItaubankTransferResponse, _> =
        res.response.parse_struct("ItaubankTransferResponse");

    match response {
        Ok(transfer_res) => {
            event_builder.map(|i| i.set_connector_response(&transfer_res));
            Ok(RouterDataV2 {
                response: Ok(PayoutTransferResponse {
                    merchant_payout_id: None,
                    payout_status: transfer_res.status(),
                    connector_payout_id: Some(transfer_res.id),
                    status_code: res.status_code,
                }),
                ..data.clone()
            })
        }
        Err(_) => Err(
            errors::ConnectorResponseTransformationError::ResponseDeserializationFailed {
                context: Default::default(),
            }
            .into(),
        ),
    }
}
```

### Example 5: Status enum mapping (no hardcoded status)

```rust
// From crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:258-274
impl ItaubankTransferResponse {
    pub fn status(&self) -> common_enums::PayoutStatus {
        match self.transfer_status {
            Some(ItaubankPayoutStatus::Aprovado)
            | Some(ItaubankPayoutStatus::Confirmado)
            | Some(ItaubankPayoutStatus::Efetivado)
            | Some(ItaubankPayoutStatus::Sucesso) => common_enums::PayoutStatus::Success,
            Some(ItaubankPayoutStatus::Pendente) | Some(ItaubankPayoutStatus::EmProcessamento) => {
                common_enums::PayoutStatus::Pending
            }
            Some(ItaubankPayoutStatus::Rejeitado)
            | Some(ItaubankPayoutStatus::Cancelado)
            | Some(ItaubankPayoutStatus::NaoIncluido) => common_enums::PayoutStatus::Failure,
            Some(ItaubankPayoutStatus::Unknown) | None => common_enums::PayoutStatus::Pending,
        }
    }
}
```

## Integration Guidelines

1. **Wire up prerequisites.** Invoke `macros::create_all_prerequisites!` as at `crates/integrations/connector-integration/src/connectors/itaubank.rs:45-51`. Keep `api: []` if no shared request/response types are needed.
2. **Exclude `PayoutTransfer` from the payout macro list.** Pass `payout_flows: [PayoutCreate, PayoutGet, PayoutVoid, ...]` but **not** `PayoutTransfer` (pattern at `crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66`).
3. **Implement the marker trait.** `impl PayoutTransferV2 for Connector<T> {}` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:289-292`).
4. **Implement `ConnectorIntegrationV2<PayoutTransfer, ...>`.** Override all six methods. Use Examples 1, 2, and 4 as skeletons.
5. **Declare `ValidationTrait::should_do_access_token = true`** if the connector requires OAuth-style pre-auth (`crates/integrations/connector-integration/src/connectors/itaubank.rs:141-147`).
6. **Write the connector-local request struct.** Derive `Serialize`, mirror Portuguese/connector-local field names as at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:104-113`.
7. **Write the `TryFrom` for the request.** Convert `MinorUnit` via the appropriate amount converter, format dates, build the beneficiary object from `PayoutMethodData`. See Example 3.
8. **Write the connector-local response struct.** Derive `Deserialize`, use `serde(alias = ...)` where the connector has multiple field names (`crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:250-256`).
9. **Map `payout_status` in an `impl Response { fn status(&self) -> PayoutStatus }` method.** Never hardcode; see Example 5.
10. **Propagate `res.status_code` to `PayoutTransferResponse.status_code`** (`crates/integrations/connector-integration/src/connectors/itaubank.rs:396`).
11. **Delegate error-response parsing to `ConnectorCommon::build_error_response`** from `get_error_response_v2` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:417-423`).

## Best Practices

- **Always guard `get_access_token` with `map_err(|_| FailedToObtainAuthType)`.** Copying the pattern at `crates/integrations/connector-integration/src/connectors/itaubank.rs:335-339` keeps the error surface consistent across payout flows.
- **Reuse `ConnectorCommon::base_url` for the URL prefix**, then compose with helper functions such as `build_env_specific_endpoint` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:426-432`). Do not hardcode `req.resource_common_data.connectors.{name}.base_url` inside each flow.
- **Default unknown response statuses to `Pending`, not `Failure`.** See `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:271-272`; this avoids spurious failures when the connector adds new states.
- **Mask sensitive header values** with `into_masked()` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:349`) and wrap request-body secrets in `Secret<String>` (`crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:133-141`).
- **Log parse failures with `tracing::error!`** before returning `ResponseDeserializationFailed` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:401-406`).
- **Prefer `change_context(IntegrationError::...)` over `map_err`** when propagating amount-conversion or date-formatting errors (`crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:166-173`).

## Common Errors / Gotchas

1. **Problem**: Double-impl of `ConnectorIntegrationV2<PayoutTransfer, ...>` because both the macro and a manual block are present.
   **Solution**: Remove `PayoutTransfer` from the `payout_flows: [...]` list passed to `macros::macro_connector_payout_implementation!` (see `crates/integrations/connector-integration/src/connectors/itaubank.rs:53-66`).

2. **Problem**: `get_headers` returns an empty Authorization header because the prior token flow was skipped.
   **Solution**: Ensure `ValidationTrait::should_do_access_token` returns `true` and that `ServerAuthenticationToken` is a separate, fully-implemented flow (`crates/integrations/connector-integration/src/connectors/itaubank.rs:161-286`).

3. **Problem**: Sandbox/production URL mismatch because `test_mode` is ignored.
   **Solution**: Read `req.resource_common_data.test_mode` and branch in a helper — mirror `build_env_specific_endpoint` (`crates/integrations/connector-integration/src/connectors/itaubank.rs:426-432`).

4. **Problem**: The connector rejects the payload because `MinorUnit` was serialized numerically but the API expects a decimal string.
   **Solution**: Use `StringMajorUnitForConnector.convert(...)` in the request `TryFrom` (`crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:163-168`).

5. **Problem**: Response parsing fails intermittently because the connector alternates field names between docs versions.
   **Solution**: Use `#[serde(alias = "...", alias = "...")]` on each potentially-renamed field, as at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:252-255`.

6. **Problem**: `PayoutStatus::Failure` is returned for rows the processor is still processing asynchronously.
   **Solution**: Map both the unknown sentinel and the `Pending`/`Processing` family to `PayoutStatus::Pending`, as at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:265-272`.

## Testing Notes

### Unit-test shape

At the pinned SHA there are no in-repo unit tests targeting `PayoutTransfer`; the repository's `trustpay_mit_grpc_test_results.md`, `jpmorgan_mit_grpc_test_results.md`, and `braintree_zerodollarauth_test_results.md` are unrelated MIT-flow artefacts (see `git status`). Recommended unit-test surface for a new implementation:

- **Request `TryFrom` per payout-method variant.** One test per supported variant of `PayoutMethodData` (`crates/types-traits/domain_types/src/payouts/payout_method_data.rs:6-13`) plus one negative test for unsupported variants.
- **Amount-converter behaviour.** At least one currency with decimal places (e.g. USD) and one without (e.g. JPY).
- **Status mapper exhaustiveness.** One test per branch of the connector-status match, including the `Unknown`/`None` default branch.
- **Date formatting.** Assert the string shape `common_utils::date_time::date_as_yyyymmddthhmmssmmmz` produces (itaubank uses this at `crates/integrations/connector-integration/src/connectors/itaubank/transformers.rs:170-173`).

### Integration-test scenarios

| Scenario | Setup | Expected `payout_status` |
| -------- | ----- | ------------------------ |
| Happy path — PIX transfer | Valid sandbox access token, `PayoutMethodData::Bank(Bank::Pix(...))` | `Success` or `Pending` (mapped from connector status) |
| Missing access token | `PayoutFlowData.access_token = None` | `IntegrationError::FailedToObtainAuthType` |
| Unsupported payout method | `PayoutMethodData::Card(...)` | Request succeeds but `recebedor = None`; processor-side rejection mapped to `Failure` |
| `test_mode = Some(true)` | Sandbox env | URL prefix from `build_env_specific_endpoint` test branch; request accepted |
| Processor rejection | Sandbox triggers decline | `ErrorResponse` surfaced via `build_error_response`; `payout_status` unset |
| Malformed response body | Mocked non-JSON 2xx | `ConnectorResponseTransformationError::ResponseDeserializationFailed` |

Integration tests MUST describe real sandbox flows (`PATTERN_AUTHORING_SPEC.md` §11).

## Cross-References

- Parent index: [./README.md](./README.md)
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Sibling flow: [pattern_payout_create.md](./pattern_payout_create.md)
- Sibling flow: [pattern_payout_get.md](./pattern_payout_get.md)
- Utility helpers: [../utility_functions_reference.md](../utility_functions_reference.md)
