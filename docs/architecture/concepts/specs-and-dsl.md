# Specs and DSL

Prism uses a domain-specific language (DSL) built on Protocol Buffers and Rust Types that catches integration errors at compile time. It enforces the right thing so that AI agents and Developers can code at ease when adding new integrations or enhancements to the Prism codebase.

## The Prism DSL

### DSL for Connector Development

The `ConnectorIntegration` trait defines the contract that every connector must implement. This trait ensures consistent behavior across all payment processor integrations. Mistakes are caught early at compile time, rather than late.

**Core trait methods:**

| Method | Purpose | When Called |
|--------|---------|-------------|
| `get_headers` | Build HTTP headers for the request | Before every API call |
| `get_url` | Construct the endpoint URL | Before every API call |
| `get_request_body` | Serialize the request payload | Before every API call |
| `build_request` | Assemble the complete HTTP request | Before every API call |
| `handle_response` | Parse successful responses | After 2xx response |
| `build_error_response` | Parse error responses | After 4xx/5xx response |
| `get_connector_transaction_id` | Extract transaction ID | After successful response |

**Example implementation:**

```rust
impl ConnectorIntegration<Authorize, AuthorizeRequest, AuthorizeResponse> for Stripe {
    fn get_headers(
        &self,
        req: &AuthorizeRequest,
        connectors: &Connectors,
    ) -> CustomResult<Vec<(String, SecretString)>, errors::IntegrationError> {
        // Build authentication headers
        vec![
            ("Authorization".to_string(), format!("Bearer {}", self.api_key).into()),
            ("Content-Type".to_string(), "application/json".to_string().into()),
        ]
    }

    fn get_url(
        &self,
        _req: &AuthorizeRequest,
        connectors: &Connectors,
    ) -> CustomResult<String, errors::IntegrationError> {
        Ok(format!("{}/v1/payment_intents", self.base_url))
    }

    fn get_request_body(
        &self,
        req: &AuthorizeRequest,
    ) -> CustomResult<RequestContent, errors::IntegrationError> {
        // Transform unified request to Stripe-specific payload
        let stripe_payload = StripeAuthorizeRequest::try_from(req)?;
        Ok(RequestContent::Json(Box::new(stripe_payload)))
    }

    fn handle_response(
        &self,
        data: &AuthorizeRequest,
        event_builder: Option<&mut ConnectorEvent>,
        res: Response,
    ) -> CustomResult<PaymentsResponseData, errors::ConnectorResponseTransformationError> {
        // Parse Stripe response into unified format
        let response: StripeAuthorizeResponse = res.response?.parse_struct("StripeAuthorizeResponse")?;
        Ok(PaymentsResponseData {
            status: response.status.into(),
            connector_transaction_id: response.id,
            // ... other fields
        })
    }

    fn build_error_response(
        &self,
        res: Response,
    ) -> CustomResult<ErrorResponse, errors::ConnectorResponseTransformationError> {
        // Transform Stripe error into unified error format
        let error: StripeErrorResponse = res.response?.parse_struct("StripeErrorResponse")?;
        Ok(ErrorResponse {
            code: error.code,
            message: error.message,
            unified_code: map_stripe_error_to_unified(&error),
        })
    }
}
```


Additionally a macro system enforces that adapters implement all the required methods.

```rust
// This macro generates compile-time checks
macros::macro_connector_implementation!(
    connector: Stripe,
    flow_name: Authorize,
    http_method: Post,
    // ... other parameters
);
```

If you forget to implement `build_error_response`, the macro invocation fails at compile time with a clear error message: "Connector Stripe is missing required method build_error_response for flow Authorize".


### Protocol Buffers

Prism defines payment operations as Protocol Buffer schemas. These generate type-safe bindings in every supported language, which is the core of the unification.
It provides compile-time guarantees irrespective of the programming languages you use the SDK with.

**Proto definition:**
```protobuf
message AuthorizeRequest {
    Money amount = 1;                    // Required
    string merchant_order_id = 2;        // Required
    PaymentMethod payment_method = 3;    // Required
    CaptureMethod capture_method = 4;    // Required
    AuthenticationType authentication_type = 5;  // Optional, defaults to NO_THREE_DS
    string customer_id = 6;              // Optional
    string email = 7;                    // Optional
    string description = 8;              // Optional
    map<string, string> metadata = 9;    // Optional
    string return_url = 10;              // Optional, required for 3DS
}

message Money {
    int64 minor_amount = 1;    // Required
    string currency = 2;       // Required, ISO 4217 format
}
```