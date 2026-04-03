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

Choose based on how the connector API expects amounts:

| API Expects | Amount Type | Example Connectors |
|---|---|---|
| Integer cents (1000 for $10.00) | `MinorUnit` | Stripe, Adyen |
| String cents ("1000" for $10.00) | `StringMinorUnit` | PayU, some legacy APIs |
| String dollars ("10.00" for $10.00) | `StringMajorUnit` | Older banking APIs |

The `CurrencyUnit` in `ConnectorCommon` must match: `MinorUnit` -> `CurrencyUnit::Minor`, `StringMajorUnit` -> `CurrencyUnit::Major`.

---

## Authentication Patterns

### HeaderKey (Bearer Token) -- most modern APIs

```rust
pub struct {ConnectorName}AuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorAuthType> for {ConnectorName}AuthType {
    type Error = IntegrationError;
    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::HeaderKey { api_key } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType { context: Default::default() }),
        }
    }
}

// In get_auth_header:
Ok(vec![(
    "Authorization".to_string(),
    format!("Bearer {}", auth.api_key.peek()).into_masked(),
)])
```

### SignatureKey (Basic Auth) -- API key + secret

```rust
impl TryFrom<&ConnectorAuthType> for {ConnectorName}AuthType {
    type Error = IntegrationError;
    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::SignatureKey { api_key, api_secret, .. } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType { context: Default::default() }),
        }
    }
}

// Basic auth header:
let credentials = format!("{}:{}", self.api_key.peek(), self.api_secret.peek());
let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, credentials);
Ok(vec![("Authorization".to_string(), format!("Basic {encoded}").into_masked())])
```

### BodyKey (Form-based) -- credentials in request body

Match `ConnectorAuthType::BodyKey { api_key, key1 }` and include credentials in the request body instead of headers.

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
                "Payment method not supported".to_string(, Default::default())).into()),
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
}

impl From<{ConnectorName}PaymentStatus> for common_enums::AttemptStatus {
    fn from(status: {ConnectorName}PaymentStatus) -> Self {
        match status {
            {ConnectorName}PaymentStatus::Succeeded => Self::Charged,
            {ConnectorName}PaymentStatus::Pending => Self::Pending,
            {ConnectorName}PaymentStatus::Failed => Self::Failure,
            {ConnectorName}PaymentStatus::RequiresAction => Self::AuthenticationPending,
            {ConnectorName}PaymentStatus::Canceled => Self::Voided,
        }
    }
}
```

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
    pub error: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<{ConnectorName}AuthorizeResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}AuthorizeResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        let status = common_enums::AttemptStatus::from(response.status.clone());

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: response.reference.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
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

```rust
fn build_error_response(
    &self,
    res: Response,
    event_builder: Option<&mut ConnectorEvent>,
) -> CustomResult<ErrorResponse, errors::ConnectorResponseTransformationError> {
    let response: {ConnectorName}ErrorResponse = if res.response.is_empty() {
        {ConnectorName}ErrorResponse::default()
    } else {
        res.response
            .parse_struct("ErrorResponse")
            .change_context(errors::ConnectorResponseTransformationError::ResponseDeserializationFailed { context: Default::default() })?
    };

    if let Some(i) = event_builder {
        i.set_error_response_body(&response);
    }

    Ok(ErrorResponse {
        status_code: res.status_code,
        code: response.error_code.unwrap_or_default(),
        message: response.error_message.unwrap_or_default(),
        reason: response.error_description,
        attempt_status: None,
        connector_transaction_id: response.transaction_id,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
    })
}
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
        auth_type: &ConnectorAuthType,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = transformers::{ConnectorName}AuthType::try_from(auth_type)
            .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
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
- Return `IntegrationError::NotImplemented` with a specific message for unsupported payment methods.
- Remove struct fields that are always `None` -- keep request/response types minimal.
- Check `utility_functions_reference.md` before writing custom helpers for country codes, card formatting, phone numbers, or address parsing.
