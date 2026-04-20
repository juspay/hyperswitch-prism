# ServerAuthenticationToken Flow Implementation Patterns

## Overview

The `ServerAuthenticationToken` flow is a crucial authentication flow in the Grace-UCS connector ecosystem. It handles OAuth 2.0 token acquisition for connectors that require bearer tokens to authenticate API requests. This pattern is typically invoked before payment flows when `should_do_access_token()` returns `true`.

### When to Use ServerAuthenticationToken

- **OAuth-based connectors**: When the connector API uses OAuth 2.0 for authentication
- **Token expiration**: When stored access tokens have expired and need refresh
- **First-time authentication**: When no valid access token exists for the connector

## Architecture

```mermaid
flowchart TB
    subgraph "ServerAuthenticationToken Flow"
        A[Payment Flow Triggered] --> B{should_do_access_token?}
        B -->|Yes| C[ServerAuthenticationToken Request]
        B -->|No| D[Skip to Payment Flow]
        C --> E[Build Token Request]
        E --> F[Send to Connector]
        F --> G[Parse Token Response]
        G --> H[Store Access Token]
        H --> I[Proceed with Payment Flow]
    end
```

## Core Components

### 1. Flow Definition

The ServerAuthenticationToken flow is defined in `domain_types::connector_flow::ServerAuthenticationToken`:

```rust
pub struct ServerAuthenticationToken;

impl ConnectorFlow for ServerAuthenticationToken {
    type Request = ServerAuthenticationTokenRequestData;
    type Response = ServerAuthenticationTokenResponseData;
}
```

### 2. Data Types

#### Request Data
```rust
// ServerAuthenticationTokenRequestData - Empty struct, auth details come from connector_auth_type
pub struct ServerAuthenticationTokenRequestData;
```

#### Response Data
```rust
pub struct ServerAuthenticationTokenResponseData {
    pub access_token: Secret<String>,
    pub expires_in: Option<i64>,      // Token expiration in seconds
    pub token_type: Option<String>,   // e.g., "Bearer"
}
```

### 3. Trait Implementation

Connectors implement `ServerAuthentication` trait to enable this flow:

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for ConnectorName<T>
{
}
```

Additionally, connectors must implement `ValidationTrait` to indicate when access token is needed:

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for ConnectorName<T>
{
    fn should_do_access_token(&self, _payment_method: common_enums::PaymentMethod) -> bool {
        true  // Return true if access token is required
    }
}
```

## Implementation Patterns

### Pattern 1: OAuth 2.0 Client Credentials Grant (Full Implementation)

Used by connectors like **Volt**, **Airwallex**, **Getnet**, **Jpmorgan**, **Trustpay**.

#### Request Structure

```rust
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConnectorAuthUpdateRequest {
    grant_type: String,           // "client_credentials" or "password"
    client_id: Secret<String>,
    client_secret: Secret<String>,
    // Optional fields depending on connector
    username: Secret<String>,     // For password grant
    password: Secret<String>,     // For password grant
    scope: Option<String>,        // OAuth scopes
}
```

#### Response Structure

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConnectorAuthUpdateResponse {
    pub access_token: Secret<String>,
    pub token_type: String,       // "Bearer"
    pub expires_in: i64,          // Seconds until expiration
    pub scope: Option<String>,    // Granted scopes (optional)
}
```

#### Example: Volt Implementation

**Step 1: Define Auth Type**
```rust
#[derive(Debug, Clone)]
pub struct VoltAuthType {
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
    pub username: Secret<String>,
    pub password: Secret<String>,
}

impl TryFrom<&ConnectorAuthType> for VoltAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        if let ConnectorAuthType::SignatureKey {
            api_key,
            key1,
            api_secret,
            key2,
        } = auth_type
        {
            Ok(Self {
                client_id: api_key.clone(),
                client_secret: api_secret.clone(),
                username: key1.clone(),
                password: key2.clone(),
            })
        } else {
            Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType { context: Default::default() }
            ))
        }
    }
}
```

**Step 2: Define Request Type**
```rust
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VoltAuthUpdateRequest {
    grant_type: String,
    client_id: Secret<String>,
    client_secret: Secret<String>,
    username: Secret<String>,
    password: Secret<String>,
}

impl TryFrom<&ConnectorAuthType> for VoltAuthUpdateRequest {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        let auth = VoltAuthType::try_from(auth_type)?;
        Ok(Self {
            grant_type: "password".to_string(),
            username: auth.username,
            password: auth.password,
            client_id: auth.client_id,
            client_secret: auth.client_secret,
        })
    }
}

// Router data conversion
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        VoltRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for VoltAuthUpdateRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: VoltRouterData<...>,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data.connector_auth_type)
    }
}
```

**Step 3: Define Response Type**
```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoltAuthUpdateResponse {
    pub access_token: Secret<String>,
    pub token_type: String,
    pub expires_in: i64,
}

// Response conversion to domain type
impl<F, T> TryFrom<ResponseRouterData<VoltAuthUpdateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<VoltAuthUpdateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                expires_in: Some(item.response.expires_in),
                token_type: Some(item.response.token_type),
            }),
            ..item.router_data
        })
    }
}
```

**Step 4: Register in Macro**
```rust
macros::create_all_prerequisites!(
    connector_name: Volt,
    generic_type: T,
    api: [
        (
            flow: ServerAuthenticationToken,
            request_body: VoltAuthUpdateRequest,
            response_body: VoltAuthUpdateResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        // ... other flows
    ]
);
```

### Pattern 2: Empty Request Body (Airwallex)

Some connectors like **Airwallex** require an empty request body for token generation:

```rust
// Empty request body for ServerAuthenticationToken - Airwallex requires empty JSON object {}
#[derive(Debug, Serialize)]
pub struct AirwallexAccessTokenRequest {
    // Empty struct that serializes to {} - Airwallex API requirement
}

// Auth is passed via Basic Auth header (client_id:client_secret base64 encoded)
```

**Key Characteristics:**
- Uses `BodyKey` auth type (api_key + key1)
- Client ID and secret sent via HTTP Basic Authentication header
- Empty JSON body `{}` in request

### Pattern 3: OAuth with Base64 Encoding (PayPal)

**PayPal** uses a specific authentication approach with Base64 encoding:

```rust
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

// Auth header generation
fn auth_headers(
    client_id: &Secret<String>,
    client_secret: &Secret<String>,
) -> CustomResult<String, IntegrationError> {
    let auth = format!(
        "{}:{}",
        client_id.expose(),
        client_secret.expose()
    );
    Ok(BASE64_ENGINE.encode(auth))
}

// Request uses form-urlencoded body
#[derive(Debug, Serialize)]
pub struct PaypalAuthUpdateRequest {
    grant_type: String,
    scope: Option<String>,
}

// Response includes additional fields
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaypalAuthUpdateResponse {
    pub access_token: Secret<String>,
    pub token_type: String,
    pub expires_in: i64,
    pub scope: String,
}
```

### Pattern 4: Stub Implementation

Most connectors (60+) use stub implementations when OAuth is not required:

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for ConnectorName<T>
{
    fn should_do_access_token(&self, _payment_method: common_enums::PaymentMethod) -> bool {
        false  // OAuth not required
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for ConnectorName<T>
{
}
```

## Request/Response Conversion Matrix

| Connector | Grant Type | Auth Header | Request Body | Auth Type |
|-----------|------------|-------------|--------------|-----------|
| Airwallex | Implicit (Basic Auth) | Basic base64(client_id:client_secret) | `{}` | BodyKey |
| Getnet | Client Credentials | Basic base64(client_id:client_secret) | Form params | BodyKey |
| Iatapay | Client Credentials | None | JSON body | SignatureKey |
| Jpmorgan | Client Credentials | None | JSON body | BodyKey |
| Paypal | Client Credentials | Basic base64(client_id:client_secret) | Form params | BodyKey |
| Trustpay | Password Grant | None | Form params | SignatureKey |
| Volt | Password Grant | None | Form params | SignatureKey |

## Error Handling Patterns

### Common Error Scenarios

```rust
// Failed to obtain auth type
Err(error_stack::report!(
    errors::IntegrationError::FailedToObtainAuthType { context: Default::default() }
))

// Missing required fields
Err(errors::IntegrationError::MissingRequiredField {
    field_name: "client_id",
, context: Default::default() })
.into())
```

### Error Response Handling

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorTokenErrorResponse {
    pub error: String,           // OAuth error code
    pub error_description: String,
}

// Convert to domain error response
impl<F, T> TryFrom<ResponseRouterData<ConnectorTokenErrorResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ConnectorTokenErrorResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Err(ErrorResponse {
                code: item.response.error,
                message: item.response.error_description,
                ..Default::default()
            }),
            ..item.router_data
        })
    }
}
```

## URL Patterns

### Sandbox vs Production

```rust
fn get_url(
    &self,
    _req: &RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
    _connectors: &Connectors,
) -> CustomResult<String, errors::IntegrationError> {
    let base_url = self.base_url(_connectors);
    Ok(format!("{}/oauth/token", base_url))
}
```

Common token endpoint patterns:
- `/oauth/token` - Standard OAuth 2.0
- `/v1/oauth2/token` - PayPal style
- `/api/v1/token` - Custom endpoints

## Header Patterns

### Content-Type Headers

| Connector | Content-Type |
|-----------|--------------|
| Airwallex | `application/json` |
| Getnet | `application/x-www-form-urlencoded` |
| Paypal | `application/x-www-form-urlencoded` |
| Trustpay | `application/x-www-form-urlencoded` |
| Volt | `application/x-www-form-urlencoded` |

### Authorization Headers

```rust
// Basic Auth pattern
pub fn get_headers(
    &self,
    req: &RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
    _connectors: &Connectors,
) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
    let auth = ConnectorAuthType::try_from(&req.connector_auth_type)?;
    let credentials = format!("{}:{}", auth.client_id.expose(), auth.client_secret.expose());
    let encoded = BASE64_ENGINE.encode(credentials);

    Ok(vec![
        (
            headers::AUTHORIZATION.to_string(),
            format!("Basic {}", encoded).into_masked(),
        ),
        (
            headers::CONTENT_TYPE.to_string(),
            "application/x-www-form-urlencoded".to_string().into_masked(),
        ),
    ])
}
```

## Testing Patterns

### Unit Test Example

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_update_request_conversion() {
        let auth_type = ConnectorAuthType::SignatureKey {
            api_key: Secret::new("client_id".to_string()),
            key1: Secret::new("username".to_string()),
            api_secret: Secret::new("client_secret".to_string()),
            key2: Secret::new("password".to_string()),
        };

        let request = VoltAuthUpdateRequest::try_from(&auth_type).unwrap();

        assert_eq!(request.grant_type, "password");
        assert_eq!(request.client_id.expose(), "client_id");
        assert_eq!(request.username.expose(), "username");
    }

    #[test]
    fn test_auth_update_response_conversion() {
        let response = VoltAuthUpdateResponse {
            access_token: Secret::new("test_token".to_string()),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
        };

        // Verify conversion to ServerAuthenticationTokenResponseData
        let domain_response: ServerAuthenticationTokenResponseData = response.into();
        assert_eq!(domain_response.token_type, Some("Bearer".to_string()));
        assert_eq!(domain_response.expires_in, Some(3600));
    }
}
```

## Integration Guidelines

### Step-by-Step Implementation

1. **Identify Auth Type**: Determine which `ConnectorAuthType` variant holds OAuth credentials
   - `BodyKey { api_key, key1 }` - Common for client_id/client_secret
   - `SignatureKey { api_key, key1, api_secret, key2 }` - For 4-field auth

2. **Create Auth Type Struct**: Define a struct to hold parsed credentials
   ```rust
   pub struct ConnectorAuthType {
       pub client_id: Secret<String>,
       pub client_secret: Secret<String>,
   }
   ```

3. **Implement TryFrom for Auth Type**:
   ```rust
   impl TryFrom<&ConnectorAuthType> for YourAuthType { ... }
   ```

4. **Create Request/Response Types**: Define serializable/deserializable structs

5. **Implement Conversions**:
   - `TryFrom<&ConnectorAuthType>` for request
   - `TryFrom<RouterData<...>>` for request
   - `TryFrom<ResponseRouterData<...>>` for response

6. **Register in Macro**: Add to `create_all_prerequisites!` macro call

7. **Enable Validation**: Implement `ValidationTrait::should_do_access_token`

8. **Implement ServerAuthentication**: Add empty trait impl

## Common Pitfalls

### 1. Auth Type Mismatch
Ensure your `TryFrom<&ConnectorAuthType>` handles the correct variant:
```rust
// WRONG - This will fail at runtime if wrong variant is used
if let ConnectorAuthType::BodyKey { ... } = auth_type { ... }

// CORRECT - Return proper error
match auth_type {
    ConnectorAuthType::BodyKey { ... } => Ok(...),
    _ => Err(error_stack::report!(
        errors::IntegrationError::FailedToObtainAuthType { context: Default::default() }
    )),
}
```

### 2. Missing Expires In
Always handle optional `expires_in`:
```rust
// Good - Handle missing expiration
pub expires_in: Option<i64>,

// Bad - Assumes always present
pub expires_in: i64,
```

### 3. Secret Exposure
Always use `Secret<String>` for sensitive data:
```rust
// Good
pub access_token: Secret<String>,

// Bad - Token will be logged!
pub access_token: String,
```

## References

- [OAuth 2.0 Specification](https://tools.ietf.org/html/rfc6749)
- [Pattern: Authorize Flow](./pattern_authorize.md)
- [Pattern: PSync Flow](./pattern_psync.md)
- [Grace-UCS Architecture](./ucs-architecture.md)

## Full Implementation Examples

### Volt (Password Grant)
See: `connectors/volt/transformers.rs` lines 308-381

### Airwallex (Empty Body + Basic Auth)
See: `connectors/airwallex/transformers.rs` lines 54-65

### PayPal (Client Credentials + Base64)
See: `connectors/paypal/transformers.rs` lines 1274+

## Summary

The ServerAuthenticationToken flow follows a consistent pattern across all connectors:

1. **Define types** for authentication credentials
2. **Implement conversions** from domain types to connector-specific types
3. **Register in macro** for automatic trait implementation
4. **Enable validation** to trigger token acquisition when needed

The key variations are:
- **Grant type**: `client_credentials`, `password`, or implicit
- **Auth transport**: HTTP Basic header vs request body
- **Content type**: JSON vs form-urlencoded
- **Request body**: Empty vs populated with credentials

## Mapping to connector_flow.rs token markers

This pattern canonically documents **ServerAuthenticationToken** as an umbrella term. The connector-service flow registry at `crates/types-traits/domain_types/src/connector_flow.rs` (SHA `60540470cf84a350cc02b0d41565e5766437eb95`) declares THREE related token markers. This section clarifies how each maps to grace patterns.

| Flow marker | Purpose | Canonical grace pattern |
|-------------|---------|--------------------------|
| `ServerSessionAuthenticationToken` (`crates/types-traits/domain_types/src/connector_flow.rs:38`) | Server-to-server session-token acquisition: short-lived per-transaction session token keyed on amount/currency/browser-info, not a reusable OAuth bearer. Request type `ServerSessionAuthenticationTokenRequestData` carries `amount`, `currency`, `browser_info` (`crates/types-traits/domain_types/src/connector_types.rs:1677-1681`); response type `ServerSessionAuthenticationTokenResponseData` returns a single `session_token: String` (`crates/types-traits/domain_types/src/connector_types.rs:1691-1694`). | [pattern_server_session_authentication_token.md](./pattern_server_session_authentication_token.md) |
| `ServerAuthenticationToken` (`crates/types-traits/domain_types/src/connector_flow.rs:41`) | Server-side OAuth 2.0 bearer-token acquisition. This is the LIVE umbrella marker used by OAuth bearer-token flows. Request type `ServerAuthenticationTokenRequestData` carries only a `grant_type: String` (`crates/types-traits/domain_types/src/connector_types.rs:1697-1699`); response type `ServerAuthenticationTokenResponseData` carries `access_token: Secret<String>`, `token_type: Option<String>`, `expires_in: Option<i64>` (`crates/types-traits/domain_types/src/connector_types.rs:1701-1706`). | this file |
| `ClientAuthenticationToken` (`crates/types-traits/domain_types/src/connector_flow.rs:62`) | Client-facing session/authentication token generation (typically for wallet/SDK initialization like Stripe PaymentIntents, Adyen sessions, Braintree client tokens). Request type `ClientAuthenticationTokenRequestData` is amount/currency/order-shaped (`crates/types-traits/domain_types/src/connector_types.rs:1596-1607`); the response type bound in macro registrations is `PaymentsResponseData`, not a dedicated `ClientAuthenticationTokenResponseData` (verified: no such struct exists in `crates/types-traits/domain_types/src/connector_types.rs`). | [pattern_client_authentication_token.md](./pattern_client_authentication_token.md) |

### Honesty note on naming

`ServerAuthenticationToken` is not itself a `pub struct` marker in `connector_flow.rs` at the pinned SHA — verified by grep against `crates/types-traits/domain_types/src/connector_flow.rs` (no match for `ServerAuthenticationToken` anywhere in the file; all flow markers are enumerated at lines 2-95). The historical title on this file is an umbrella term. The live marker that backs OAuth bearer-token acquisition is `ServerAuthenticationToken` at `crates/types-traits/domain_types/src/connector_flow.rs:41`. Readers implementing a new OAuth flow should register `ServerAuthenticationToken`, not a non-existent `ServerAuthenticationToken`.

### Request/response types

The three markers bind **distinct** request/response data types — they are NOT shared:

- `ServerSessionAuthenticationToken` → `ServerSessionAuthenticationTokenRequestData` / `ServerSessionAuthenticationTokenResponseData` (`crates/types-traits/domain_types/src/connector_types.rs:1677`, `crates/types-traits/domain_types/src/connector_types.rs:1692`)
- `ServerAuthenticationToken` → `ServerAuthenticationTokenRequestData` / `ServerAuthenticationTokenResponseData` (`crates/types-traits/domain_types/src/connector_types.rs:1697`, `crates/types-traits/domain_types/src/connector_types.rs:1702`)
- `ClientAuthenticationToken` → `ClientAuthenticationTokenRequestData` / `PaymentsResponseData` (`crates/types-traits/domain_types/src/connector_types.rs:1596`; response binding verified in real registrations, e.g. `crates/integrations/connector-integration/src/connectors/braintree.rs:352`, `crates/integrations/connector-integration/src/connectors/adyen.rs:301`). There is no `ClientAuthenticationTokenResponseData` struct in `connector_types.rs`.

### Worked examples from source

Real connector registrations were enumerated by grepping `crates/integrations/connector-integration/src/connectors/` for each marker's `flow: <marker>,` macro line inside `create_all_prerequisites!`.

- `ServerAuthenticationToken`: **11 registrations** — Jpmorgan (`crates/integrations/connector-integration/src/connectors/jpmorgan.rs:281`), Getnet (`crates/integrations/connector-integration/src/connectors/getnet.rs:256`), Truelayer (`crates/integrations/connector-integration/src/connectors/truelayer.rs:258`), Iatapay (`crates/integrations/connector-integration/src/connectors/iatapay.rs:259`), Volt (`crates/integrations/connector-integration/src/connectors/volt.rs:257`), Airwallex (`crates/integrations/connector-integration/src/connectors/airwallex.rs:258`), Fiservcommercehub (`crates/integrations/connector-integration/src/connectors/fiservcommercehub.rs:78`), Trustpay (`crates/integrations/connector-integration/src/connectors/trustpay.rs:466`), Paypal (`crates/integrations/connector-integration/src/connectors/paypal.rs:526`), Pinelabs_online (`crates/integrations/connector-integration/src/connectors/pinelabs_online.rs:263`).
- `ServerSessionAuthenticationToken`: **2 registrations** (narrower usage) — Paytm (`crates/integrations/connector-integration/src/connectors/paytm.rs:60`) and Nuvei (`crates/integrations/connector-integration/src/connectors/nuvei.rs:216`).
- `ClientAuthenticationToken`: **18+ registrations** (wider usage post-#1002) — including Braintree (`crates/integrations/connector-integration/src/connectors/braintree.rs:349`), Adyen (`crates/integrations/connector-integration/src/connectors/adyen.rs:298`), Stripe (`crates/integrations/connector-integration/src/connectors/stripe.rs:311`), Cybersource (`crates/integrations/connector-integration/src/connectors/cybersource.rs:297`), Paypal (`crates/integrations/connector-integration/src/connectors/paypal.rs:555`), Bluesnap (`crates/integrations/connector-integration/src/connectors/bluesnap.rs:433`), Jpmorgan (`crates/integrations/connector-integration/src/connectors/jpmorgan.rs:321`), and others (Rapyd, Globalpay, Nexinets, Payload, Mollie, Datatrans, Shift4, Billwerk, Multisafepay, Nuvei, Nexixpay).

### Cross-references

- [pattern_server_session_authentication_token.md](./pattern_server_session_authentication_token.md)
- [pattern_client_authentication_token.md](./pattern_client_authentication_token.md)
- [PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)

## Change Log

| Version | Date | Change |
|---------|------|--------|
| 1.2.1 | 2026-04-20 | Absorbed PR #855 auth-token rename (commit `c9e1025e3`): file renamed from `pattern_CreateAccessToken_flow.md` → `pattern_server_authentication_token.md`; `CreateAccessToken` → `ServerAuthenticationToken`, `PaymentAccessToken` → `ServerAuthentication`, `AccessTokenRequestData` → `ServerAuthenticationTokenRequestData`, `AccessTokenResponseData` → `ServerAuthenticationTokenResponseData` throughout. |
| 1.2.0 | 2026-04-20 | Added "Mapping to connector_flow.rs token markers" section disambiguating `ServerAuthenticationToken`, `ServerSessionAuthenticationToken`, and `ClientAuthenticationToken` with file:line citations against SHA `60540470cf84a350cc02b0d41565e5766437eb95`; added header metadata table. |
