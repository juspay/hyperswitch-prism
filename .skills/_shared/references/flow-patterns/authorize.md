# Authorize Flow Pattern

The authorize flow receives payment authorization requests, transforms them to connector-specific format, sends requests to the gateway, and maps responses back to standardized types.

For macro syntax details, see `macro-reference.md`.
For utility functions (country codes, card formatting, phone numbers), see `utility_functions_reference.md`.

---

## File Structure

```
connectors/
├── {connector_name}.rs              # Main connector implementation
└── {connector_name}/
    └── transformers.rs              # Request/response data transformations
```

---

## Amount Type Selection

**Read the vendor spec and match its wire format.** There is no safe default -- picking
one "because it is common" is wrong most of the time. All five types live in
`crates/common/common_utils/src/types.rs`.

| API Expects | Amount Type | Example |
|---|---|---|
| Integer minor units (`1000` for $10.00) | `MinorUnit` | `"amount": 1000` |
| String minor units (`"1000"` for $10.00) | `StringMinorUnit` | `"amount": "1000"` |
| String major units (`"10.00"` for $10.00) | `StringMajorUnit` | `"amount": "10.00"` |
| Float major units (`10.00` for $10.00) | `FloatMajorUnit` | `"amount": 10.00` |
| String major units, always 2 decimals | `StringTwoDecimalUnit` | `"amount": "10.00"` |

Actual distribution across connectors on HEAD: `StringMajorUnit` 34, `FloatMajorUnit` 26,
`MinorUnit` 21, `StringMinorUnit` 19. If the spec shows a quoted decimal string, it is
`StringMajorUnit` (or `StringTwoDecimalUnit` when the spec mandates exactly two decimals),
not `StringMinorUnit`.

The `CurrencyUnit` in `ConnectorCommon` must match: `MinorUnit` / `StringMinorUnit` ->
`CurrencyUnit::Minor`; the major-unit types -> `CurrencyUnit::Base`.

---

## Authentication Patterns

`ConnectorAuthType` no longer exists on this contract. `RouterDataV2::connector_auth_type`
was removed on 2026-03-14; credentials now arrive on `req.connector_config`, typed as
`domain_types::router_data::ConnectorSpecificConfig` -- an enum with **one variant per
connector**. Adding a connector means adding its variant there and matching on it.

Working exemplar: `crates/integrations/connector-integration/src/connectors/travelhub.rs`
(+ `travelhub/transformers.rs`).

### Single-credential (Bearer token)

```rust
pub struct {ConnectorName}AuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for {ConnectorName}AuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::{ConnectorName} { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Configure the connector account with {ConnectorName} credentials"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "ConnectorSpecificConfig variant mismatch: expected {ConnectorName} credentials"
                                .to_string(),
                        ),
                    },
                }
            )),
        }
    }
}

// In get_auth_header:
Ok(vec![(
    headers::AUTHORIZATION.to_string(),
    format!("Bearer {}", auth.api_key.peek()).into_masked(),
)])
```

### Multi-credential (Basic auth from username + password)

Same shape -- destructure the extra fields out of your own variant, then build the header
on the auth struct:

```rust
impl {ConnectorName}AuthType {
    pub fn generate_authorization_header(&self) -> String {
        let credentials = format!("{}:{}", self.username.peek(), self.password.peek());
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes())
        )
    }
}
```

### Credentials in the request body

Destructure the same `ConnectorSpecificConfig::{ConnectorName}` variant inside the request
`TryFrom` (from `item.router_data.connector_config`) and serialize the fields into the body
instead of a header.

---

## Request Structure and TryFrom

### Request Types

```rust
#[derive(Debug, Serialize)]
pub struct {ConnectorName}AuthorizeRequest<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> {
    pub amount: {AmountType},
    pub currency: String,
    pub payment_method: {ConnectorName}PaymentMethod<T>,
    pub reference: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum {ConnectorName}PaymentMethod<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> {
    Card({ConnectorName}Card<T>),
}

#[derive(Debug, Serialize)]
pub struct {ConnectorName}Card<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> {
    pub number: RawCardNumber<T>,
    pub exp_month: Secret<String>,
    pub exp_year: Secret<String>,
    pub cvc: Option<Secret<String>>,
}
```

### TryFrom for Request (Payment Method Data Extraction)

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<{ConnectorName}RouterData<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>>
    for {ConnectorName}AuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(item: {ConnectorName}RouterData<...>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let payment_method = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                {ConnectorName}PaymentMethod::Card({ConnectorName}Card {
                    number: card_data.card_number.clone(),
                    exp_month: card_data.card_exp_month.clone(),
                    exp_year: card_data.card_exp_year.clone(),
                    cvc: Some(card_data.card_cvc.clone()),
                })
            },
            _ => return Err(IntegrationError::NotImplemented(
                "Payment method not supported".to_string(),
                IntegrationErrorContext::default(),
            ).into()),
        };
        Ok(Self {
            amount: item.amount,
            currency: router_data.request.currency.to_string(),
            payment_method,
            reference: router_data.resource_common_data.connector_request_reference_id.clone(),
        })
    }
}
```

### RouterData Helper Struct

Wraps `RouterDataV2` with the converted amount. Implements `TryFrom<({AmountType}, T, U)>` to construct from the tuple of `(converted_amount, router_data, connector)`. The macro framework generates this automatically.

---

## Response Structure and Status Mapping

### WARNING: NEVER HARDCODE STATUS VALUES

Always derive payment status from the connector's actual response. Hardcoding `AttemptStatus::Charged` is a critical bug -- the payment may have failed, be pending for 3DS, or be in any other state.

### Status Enum and From Implementation

```rust
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum {ConnectorName}PaymentStatus {
    Succeeded,
    Pending,
    Failed,
    RequiresAction,
    Canceled,
    /// Absorbs any status string the vendor adds later. Without this a new status is a
    /// hard deserialization failure instead of a safe non-terminal state.
    #[serde(other)]
    Unknown,
}

impl From<{ConnectorName}PaymentStatus> for common_enums::AttemptStatus {
    fn from(status: {ConnectorName}PaymentStatus) -> Self {
        match status {
            {ConnectorName}PaymentStatus::Succeeded => Self::Charged,
            {ConnectorName}PaymentStatus::Pending => Self::Pending,
            {ConnectorName}PaymentStatus::Failed => Self::Failure,
            {ConnectorName}PaymentStatus::RequiresAction => Self::AuthenticationPending,
            {ConnectorName}PaymentStatus::Canceled => Self::Voided,
            {ConnectorName}PaymentStatus::Unknown => Self::Pending,
        }
    }
}
```

**Both halves are required, and reviewers check for both:**

1. `#[serde(other)] Unknown` at the **deserialization** layer -- an unrecognised wire value
   must land in a named variant, not blow up the response parse.
2. An **exhaustive** `match` at the **status-mapping** layer -- never a catch-all `_ =>`.
   A catch-all silently swallows statuses the vendor adds later and maps them to whatever
   the arm happens to say; the compiler can no longer tell you a variant went unhandled.

### Manual Capture Awareness

When a connector uses the same status for both authorized and captured states:

```rust
fn map_status(status: &{ConnectorName}PaymentStatus, is_manual_capture: bool) -> common_enums::AttemptStatus {
    match status {
        {ConnectorName}PaymentStatus::Succeeded => {
            if is_manual_capture {
                common_enums::AttemptStatus::Authorized
            } else {
                common_enums::AttemptStatus::Charged
            }
        },
        {ConnectorName}PaymentStatus::Pending => common_enums::AttemptStatus::Pending,
        {ConnectorName}PaymentStatus::Failed => common_enums::AttemptStatus::Failure,
        // ...
    }
}
```

### Response TryFrom Implementation

```rust
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}AuthorizeResponse {
    pub id: String,
    pub status: {ConnectorName}PaymentStatus,
    pub amount: Option<i64>,
    pub reference: Option<String>,
    /// Present only on in-band failures; drives the `Err(ErrorResponse)` branch below.
    pub error: Option<String>,
    pub error_code: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<{ConnectorName}AuthorizeResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}AuthorizeResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        let status = common_enums::AttemptStatus::from(response.status.clone());

        // In-band failure: a 2xx body that carries a declined/failed status must become
        // `response: Err(ErrorResponse { .. })`, not an `Ok` with a Failure status. Branch on
        // `domain_types::utils::is_payment_failure` rather than on the HTTP code.
        if domain_types::utils::is_payment_failure(status) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: response
                        .error_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .error
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.error.clone(),
                    status_code: item.http_code,
                    // Flow-aware: the status just derived from the connector's own response.
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: Some(response.id.clone()),
                    ..Default::default()
                }),
                ..router_data.clone()
            });
        }

        // `PaymentsResponseData::TransactionResponse` is an enum struct-variant, so there is no
        // `..Default::default()` here -- every one of the 11 fields must be listed (E0063).
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
            redirection_data: None,
            connector_metadata: None,
            mandate_reference: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: response.reference.clone(),
            incremental_authorization_allowed: None,
            splits: None,
            status_code: item.http_code,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}
```

### `connector_metadata` is the identifier carrier

Anything PSync/Capture/Void/Refund will need later (an order id, a session handle, a second
identifier the vendor requires alongside the transaction id) must be serialized into
`connector_metadata` here. It travels back as `connector_feature_data` on the follow-up
flow's request. Leaving it `None` when the vendor needs a second identifier is the classic
identifier round-trip bug -- PSync then cannot address the payment it is syncing.

---

## Error Handling

### Error Response Structure

```rust
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}ErrorResponse {
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_description: Option<String>,
    pub transaction_id: Option<String>,
}

impl Default for {ConnectorName}ErrorResponse {
    fn default() -> Self {
        Self {
            error_code: Some("UNKNOWN_ERROR".to_string()),
            error_message: Some("Unknown error occurred".to_string()),
            error_description: None,
            transaction_id: None,
        }
    }
}
```

For connectors with multiple error formats, use `#[serde(untagged)]` enum variants.

### build_error_response in ConnectorCommon

Signature per `crates/types-traits/interfaces/src/api.rs:50` -- **three** parameters besides
`&self`. The event type is `common_utils::events::Event` (there is no `ConnectorEvent`), and
`Event` has no `set_error_response_body` method; use the `with_error_response_body!` macro
from `crate::utils` instead. The same third `_connector_config` parameter was added to
`get_error_response_v2` and `get_5xx_error_response`.

```rust
fn build_error_response(
    &self,
    res: domain_types::router_response_types::Response,
    event_builder: Option<&mut events::Event>,
    _connector_config: &ConnectorSpecificConfig,
) -> CustomResult<ErrorResponse, ConnectorError> {
    let response: {ConnectorName}ErrorResponse = if res.response.is_empty() {
        {ConnectorName}ErrorResponse::default()
    } else {
        res.response
            .parse_struct("{ConnectorName}ErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "Failed to parse the {ConnectorName} error response body".to_string(),
                    ),
                },
            })?
    };

    with_error_response_body!(event_builder, response);

    Ok(ErrorResponse {
        status_code: res.status_code,
        // Never `.unwrap_or_default()` here: an empty `code`/`message` reaches the merchant as
        // a blank error. Use the shared sentinels from `common_utils::consts`.
        code: response
            .error_code
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: response
            .error_message
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: response.error_description,
        // Leave `None` unless the connector response proves the attempt reached a terminal
        // state. Hardcoding `Some(FlowStatus::Payment(AttemptStatus::Failure))` on this shared
        // path is what reports a charged payment as FAILURE. When you *can* prove terminality,
        // set the flow-appropriate variant: `FlowStatus::Payment(..)` for payment flows,
        // `FlowStatus::Refund(..)` for refund flows -- see `connectors/flywire.rs` for the
        // full form and `connectors/noon.rs` for the minimal one.
        attempt_status: None,
        connector_transaction_id: response.transaction_id,
        ..Default::default()
    })
}
```

`ErrorResponse` has 13 fields (`domain_types::router_data`) and does implement `Default`, so
`..Default::default()` covers `network_decline_code`, `network_advice_code`,
`network_error_message`, `typed_connector_response`, `raw_connector_response`,
`raw_connector_request` and `typed_connector_request`. Note `attempt_status` is
`Option<FlowStatus>`, **not** `Option<AttemptStatus>`:

```rust
pub enum FlowStatus {
    Payment(common_enums::AttemptStatus),
    Refund(common_enums::RefundStatus),
    Dispute(common_enums::DisputeStatus),
    Payout(common_enums::PayoutStatus),
}
```

Imports these snippets rely on:

```rust
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,   // provides `parse_struct` on `&[u8]`
};
use domain_types::{
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext, ResponseTransformationErrorContext},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
};
use crate::with_error_response_body;
```

---

## URL Construction, Headers, and Request Format

**URL**: Use `self.connector_base_url_payments(req)` for the base. Append the endpoint path. For sync/capture/void, include the transaction ID in the path.

```rust
let base_url = self.connector_base_url_payments(req);
Ok(format!("{base_url}/v1/payments"))          // authorize
Ok(format!("{base_url}/v1/payments/{txn_id}")) // sync/capture/void
```

**Headers**: Build via `build_headers` helper in `create_all_prerequisites!` `member_functions`. Combines Content-Type + auth header from `get_auth_header`.

**Request format options** (set in macro `curl_request` field):
- `Json(...)` -- `application/json` (most common)
- `FormUrlEncoded(...)` -- `application/x-www-form-urlencoded` (use `#[serde(flatten)]` and `#[serde(rename = "card[number]")]`)
- XML: custom `to_xml()` method, return as `RequestContent::RawBytes`

---

## ConnectorCommon Implementation

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorCommon for {ConnectorName}<T>
{
    fn id(&self) -> &'static str { "{connector_name}" }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Minor // Must match your AmountType choice
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.{connector_name}.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = transformers::{ConnectorName}AuthType::try_from(auth_type)
            .change_context(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext::default(),
            })?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.peek()).into_masked(),
        )])
    }

    // build_error_response: see Error Handling section above
}
```

## Macro Invocation for Authorize Flow

See `macro-reference.md` for full macro syntax. Key fields for authorize:

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}AuthorizeRequest),
    curl_response: {ConnectorName}AuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>)
            -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(&self, req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>)
            -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/v1/payments"))
        }
    }
);
```

---

## Key Principles

- Status must always be mapped from the connector response via `From` trait or `match` -- never hardcoded.
- Use `Maskable` types for all sensitive data (card numbers, auth tokens). Never log PII.
- Return `IntegrationError::NotImplemented(message, IntegrationErrorContext::default())` with a
  specific message for unsupported payment methods. Note the shape: it is a **tuple** variant
  taking `(String, IntegrationErrorContext)`. `ConnectorError` has only five variants
  (`ResponseDeserializationFailed`, `ResponseHandlingFailed`, `UnexpectedResponseError`,
  `IntegrityCheckFailed`, `ConnectorErrorResponse`) -- request-side errors are always
  `IntegrationError`.
- A 2xx response carrying a declined status is still a failure: return
  `response: Err(ErrorResponse { .. })`, gated on `domain_types::utils::is_payment_failure`.
- Remove struct fields that are always `None` -- keep request/response types minimal.
- Check `utility_functions_reference.md` before writing custom helpers for country codes, card formatting, phone numbers, or address parsing.
