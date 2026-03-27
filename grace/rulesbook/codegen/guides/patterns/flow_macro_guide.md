# Flow Implementation Using Macros - Quick Reference Guide

This guide provides flow-specific macro implementation patterns. Use this alongside individual flow pattern files.

## Quick Start: 3-Step Macro Implementation

### Step 1: Add Flow to `create_all_prerequisites!`

```rust
macros::create_all_prerequisites!(
    connector_name: {{ConnectorName}},
    generic_type: T,
    api: [
        (
            flow: {{FlowName}},
            request_body: {{ConnectorName}}{{FlowName}}Request<T>,  // Optional <T> if generic needed
            response_body: {{ConnectorName}}{{FlowName}}Response,
            router_data: RouterDataV2<{{FlowName}}, {{FlowData}}, {{RequestData}}, {{ResponseData}}>,
        ),
    ],
    amount_converters: [amount_converter: StringMinorUnit],
    member_functions: { /* helper functions */ }
);
```

### Step 2: Implement Flow with `macro_connector_implementation!`

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {{ConnectorName}},
    curl_request: Json({{ConnectorName}}{{FlowName}}Request),
    curl_response: {{ConnectorName}}{{FlowName}}Response,
    flow_name: {{FlowName}},
    resource_common_data: {{FlowData}},
    flow_request: {{RequestData}},
    flow_response: {{ResponseData}},
    http_method: {{Method}},
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<...>) -> CustomResult<...> {
            self.build_headers(req)
        }
        fn get_url(&self, req: &RouterDataV2<...>) -> CustomResult<String, ...> {
            Ok(format!("{}/endpoint", self.connector_base_url(req)))
        }
    }
);
```

### Step 3: Create Transformers

```rust
// In transformers.rs
impl<T: PaymentMethodDataTypes> TryFrom<{{ConnectorName}}RouterData<...>>
    for {{ConnectorName}}{{FlowName}}Request<T>
{
    // Build request from RouterDataV2
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<...>>
    for RouterDataV2<...>
{
    // Parse response and update RouterDataV2
}
```

---

## Flow-Specific Macro Configurations

### Authorize Flow

**Flow Definition:**
```rust
(
    flow: Authorize,
    request_body: {{ConnectorName}}PaymentRequest<T>,
    response_body: {{ConnectorName}}PaymentResponse,
    router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
),
```

**Flow Implementation:**
```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {{ConnectorName}},
    curl_request: Json({{ConnectorName}}PaymentRequest),
    curl_response: {{ConnectorName}}PaymentResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
    }
);
```

---

### PSync Flow

**Flow Definition (with request body - for POST sync):**
```rust
(
    flow: PSync,
    request_body: {{ConnectorName}}SyncRequest,
    response_body: {{ConnectorName}}SyncResponse,
    router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
),
```

**Flow Definition (without request body - for GET sync):**
```rust
(
    flow: PSync,
    response_body: {{ConnectorName}}SyncResponse,
    router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
),
```

**Flow Implementation (POST with request):**
```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {{ConnectorName}},
    curl_request: Json({{ConnectorName}}SyncRequest),
    curl_response: {{ConnectorName}}SyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!("{}/v1/payments/status", self.connector_base_url_payments(req)))
        }
    }
);
```

**Flow Implementation (GET without request):**
```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {{ConnectorName}},
    curl_response: {{ConnectorName}}SyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/v1/payments/{}", self.connector_base_url_payments(req), id))
        }
    }
);
```

---

### Capture Flow

**Flow Definition:**
```rust
(
    flow: Capture,
    request_body: {{ConnectorName}}CaptureRequest,
    response_body: {{ConnectorName}}CaptureResponse,
    router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
),
```

**Flow Implementation:**
```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {{ConnectorName}},
    curl_request: Json({{ConnectorName}}CaptureRequest),
    curl_response: {{ConnectorName}}CaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let id = match &req.request.connector_transaction_id {
                ResponseId::ConnectorTransactionId(id) => id,
                _ => return Err(errors::ConnectorError::MissingConnectorTransactionID.into())
            };
            Ok(format!("{}/v1/payments/{}/capture", self.connector_base_url_payments(req), id))
        }
    }
);
```

---

### Void Flow

**Flow Definition:**
```rust
(
    flow: Void,
    request_body: {{ConnectorName}}VoidRequest,
    response_body: {{ConnectorName}}VoidResponse,
    router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
),
```

**Flow Implementation:**
```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {{ConnectorName}},
    curl_request: Json({{ConnectorName}}VoidRequest),
    curl_response: {{ConnectorName}}VoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/v1/payments/{}/void", self.connector_base_url_payments(req), id))
        }
    }
);
```

---

### Refund Flow

**Flow Definition:**
```rust
(
    flow: Refund,
    request_body: {{ConnectorName}}RefundRequest,
    response_body: {{ConnectorName}}RefundResponse,
    router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
),
```

**Flow Implementation:**
```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {{ConnectorName}},
    curl_request: Json({{ConnectorName}}RefundRequest),
    curl_response: {{ConnectorName}}RefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!("{}/v1/refunds", self.connector_base_url_refunds(req)))
        }
    }
);
```

---

### RSync Flow

**Flow Definition:**
```rust
(
    flow: RSync,
    response_body: {{ConnectorName}}RefundResponse,
    router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
),
```

**Flow Implementation:**
```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {{ConnectorName}},
    curl_response: {{ConnectorName}}RefundResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let id = req.request.connector_refund_id.clone();
            Ok(format!("{}/v1/refunds/{}", self.connector_base_url_refunds(req), id))
        }
    }
);
```

---

## Common Patterns and Tips

### When to Use Generic `<T>`

**Use `<T>` when:**
- Request needs payment method data (Authorize, SetupMandate)
- Request structure varies by payment method

**Don't use `<T>` when:**
- Flow operates on existing transaction (Capture, Void, Sync)
- Request is simple and doesn't need payment method specifics

### Content Type Selection

| Content Type | Use Case | Example |
|--------------|----------|---------|
| `Json(Type)` | JSON API requests | Most modern APIs |
| `FormData(Type)` | Multipart form uploads | File uploads, 3DS |
| `FormUrlEncoded(Type)` | URL-encoded forms | Legacy APIs |
| Omit parameter | GET requests | Sync operations |

### Resource Common Data

| Flow Type | Use This |
|-----------|----------|
| Payment operations | `PaymentFlowData` |
| Refund operations | `RefundFlowData` |
| Dispute operations | `DisputeFlowData` |

### URL Construction Patterns

**Static endpoint:**
```rust
Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
```

**With transaction ID:**
```rust
let id = req.request.connector_transaction_id.clone();
Ok(format!("{}/v1/payments/{}", self.connector_base_url_payments(req), id))
```

**With error handling:**
```rust
let id = match &req.request.connector_transaction_id {
    ResponseId::ConnectorTransactionId(id) => id,
    _ => return Err(errors::ConnectorError::MissingConnectorTransactionID.into())
};
Ok(format!("{}/v1/payments/{}/action", self.connector_base_url_payments(req), id))
```

---

## Complete Example: Adding a New Flow

Let's add a complete Authorize flow to a connector named "ExamplePay":

### 1. Add to `create_all_prerequisites!`

```rust
macros::create_all_prerequisites!(
    connector_name: ExamplePay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: ExamplePayPaymentRequest<T>,
            response_body: ExamplePayPaymentResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
    ],
    amount_converters: [
        amount_converter: StringMinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_auth_type)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.examplepay.base_url
        }
    }
);
```

### 2. Implement with `macro_connector_implementation!`

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ExamplePay,
    curl_request: Json(ExamplePayPaymentRequest),
    curl_response: ExamplePayPaymentResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!("{}/api/v1/charge", self.connector_base_url_payments(req)))
        }
    }
);
```

### 3. Define Request/Response Types in `transformers.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ExamplePayPaymentRequest<T> {
    pub amount: String,
    pub currency: String,
    pub payment_method: ExamplePayPaymentMethod<T>,
    // ... other fields
}

#[derive(Debug, Deserialize)]
pub struct ExamplePayPaymentResponse {
    pub id: String,
    pub status: String,
    // ... other fields
}
```

### 4. Implement Transformers

```rust
impl<T: PaymentMethodDataTypes> TryFrom<ExamplePayRouterData<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>>
    for ExamplePayPaymentRequest<T>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ExamplePayRouterData<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;

        let amount = connector.amount_converter.convert(
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            amount,
            currency: router_data.request.currency.to_string(),
            payment_method: ExamplePayPaymentMethod::try_from(&router_data.request.payment_method_data)?,
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<ExamplePayPaymentResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<ExamplePayPaymentResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;

        router_data.response = PaymentsResponseData::TransactionResponse {
            connector_transaction_id: ResponseId::ConnectorTransactionId(item.response.id),
            status: get_payment_status(&item.response.status),
            // ... map other fields
        };

        Ok(router_data)
    }
}
```

---

## Validation Checklist

After implementing a flow with macros, verify:

- [ ] Flow defined in `create_all_prerequisites!` api array
- [ ] Matching `macro_connector_implementation!` block exists
- [ ] Flow name matches exactly between both macros
- [ ] Request type matches (including `<T>` if needed)
- [ ] Response type matches
- [ ] Correct `resource_common_data` used
- [ ] Correct `flow_request` and `flow_response` types
- [ ] HTTP method appropriate for operation
- [ ] `get_url` correctly constructs endpoint
- [ ] Request transformer implemented (if request body exists)
- [ ] Response transformer implemented
- [ ] Status mapping function exists
- [ ] Error handling complete

---

## See Also

- **[macro_patterns_reference.md](./macro_patterns_reference.md)** - Complete macro reference
- **[macro_templates.md](../template-generation/macro_templates.md)** - Code generation templates
- **Individual pattern files** - Flow-specific implementation details
  - pattern_authorize.md
  - pattern_psync.md
  - pattern_capture.md
  - pattern_void.md
  - pattern_refund.md
  - pattern_rsync.md
