# Grace UCS Macro-Based Code Generation Templates

This file contains templates for generating UCS connector code using the macro-based pattern.

## Template Placeholders

Use these placeholders when generating code:

- `{{CONNECTOR_NAME}}` - PascalCase connector name (e.g., `Stripe`, `Adyen`, `PayPal`)
- `{{connector_name}}` - snake_case connector name (e.g., `stripe`, `adyen`, `paypal`)
- `{{CONNECTOR_NAME_LOWER}}` - lowercase connector name (e.g., `stripe`, `adyen`, `paypal`)
- `{{FLOW_NAME}}` - PascalCase flow name (e.g., `Authorize`, `Capture`)
- `{{flow_name}}` - snake_case flow name (e.g., `authorize`, `capture`)
- `{{REQUEST_TYPE}}` - Request struct name
- `{{RESPONSE_TYPE}}` - Response struct name
- `{{HTTP_METHOD}}` - HTTP method (Post, Get, Put, etc.)
- `{{CONTENT_TYPE}}` - Content type (Json, FormData, FormUrlEncoded)
- `{{RESOURCE_COMMON_DATA}}` - Flow data type (PaymentFlowData, RefundFlowData, DisputeFlowData)
- `{{FLOW_REQUEST_DATA}}` - Request data type from domain_types
- `{{FLOW_RESPONSE_DATA}}` - Response data type from domain_types
- `{{AMOUNT_TYPE}}` - Amount converter type (StringMinorUnit, FloatMajorUnit)

## Template 1: Complete Connector File Structure

```rust
// File: crates/integrations/connector-integration/src/connectors/{{connector_name}}.rs

mod test;
pub mod transformers;

use std::{fmt::Debug, marker::{Send, Sync}, sync::LazyLock};
use common_enums::*;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::*,
    connector_types::*,
    errors,
    payment_method_data::{DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorAuthType, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::*,
    utils,
};
use error_stack::report;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self, ConnectorValidation},
};
use serde::Serialize;
use transformers::{self as {{connector_name}}, *};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    // Add connector-specific headers here
}

// ============================================================================
// TRAIT IMPLEMENTATIONS
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for {{CONNECTOR_NAME}}<T>
{}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for {{CONNECTOR_NAME}}<T>
{}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for {{CONNECTOR_NAME}}<T>
{}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for {{CONNECTOR_NAME}}<T>
{}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for {{CONNECTOR_NAME}}<T>
{}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for {{CONNECTOR_NAME}}<T>
{}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for {{CONNECTOR_NAME}}<T>
{}

// ============================================================================
// FOUNDATION SETUP - create_all_prerequisites!
// ============================================================================

macros::create_all_prerequisites!(
    connector_name: {{CONNECTOR_NAME}},
    generic_type: T,
    api: [
        {{FLOW_DEFINITIONS}}
    ],
    amount_converters: [
        amount_converter: {{AMOUNT_TYPE}}
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
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
            &req.resource_common_data.connectors.{{connector_name}}.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.{{connector_name}}.base_url
        }
    }
);

// ============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for {{CONNECTOR_NAME}}<T>
{
    fn id(&self) -> &'static str {
        "{{connector_name}}"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Minor
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorAuthType,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = {{connector_name}}::{{CONNECTOR_NAME}}AuthType::try_from(auth_type)
            .map_err(|_| errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.peek()).into_masked(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.{{connector_name}}.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: {{connector_name}}::{{CONNECTOR_NAME}}ErrorResponse = res
            .response
            .parse_struct("ErrorResponse")
            .map_err(|_| errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.clone(),
            message: response.message.clone(),
            reason: Some(response.message),
            attempt_status: None,
            connector_transaction_id: response.transaction_id,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// ============================================================================
// FLOW IMPLEMENTATIONS
// ============================================================================

{{FLOW_IMPLEMENTATIONS}}
```

## Template 2: Flow Definition (for create_all_prerequisites!)

```rust
(
    flow: {{FLOW_NAME}},
    request_body: {{REQUEST_TYPE}},
    response_body: {{RESPONSE_TYPE}},
    router_data: RouterDataV2<{{FLOW_NAME}}, {{RESOURCE_COMMON_DATA}}, {{FLOW_REQUEST_DATA}}, {{FLOW_RESPONSE_DATA}}>,
),
```

## Template 3: Flow Definition Without Request Body

```rust
(
    flow: {{FLOW_NAME}},
    response_body: {{RESPONSE_TYPE}},
    router_data: RouterDataV2<{{FLOW_NAME}}, {{RESOURCE_COMMON_DATA}}, {{FLOW_REQUEST_DATA}}, {{FLOW_RESPONSE_DATA}}>,
),
```

## Template 4: Flow Implementation (macro_connector_implementation!)

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {{CONNECTOR_NAME}},
    curl_request: {{CONTENT_TYPE}}({{REQUEST_TYPE}}),
    curl_response: {{RESPONSE_TYPE}},
    flow_name: {{FLOW_NAME}},
    resource_common_data: {{RESOURCE_COMMON_DATA}},
    flow_request: {{FLOW_REQUEST_DATA}},
    flow_response: {{FLOW_RESPONSE_DATA}},
    http_method: {{HTTP_METHOD}},
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<{{FLOW_NAME}}, {{RESOURCE_COMMON_DATA}}, {{FLOW_REQUEST_DATA}}, {{FLOW_RESPONSE_DATA}}>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<{{FLOW_NAME}}, {{RESOURCE_COMMON_DATA}}, {{FLOW_REQUEST_DATA}}, {{FLOW_RESPONSE_DATA}}>,
        ) -> CustomResult<String, errors::IntegrationError> {
            {{URL_CONSTRUCTION}}
        }
    }
);
```

## Template 5: Flow Implementation Without Request Body

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {{CONNECTOR_NAME}},
    curl_response: {{RESPONSE_TYPE}},
    flow_name: {{FLOW_NAME}},
    resource_common_data: {{RESOURCE_COMMON_DATA}},
    flow_request: {{FLOW_REQUEST_DATA}},
    flow_response: {{FLOW_RESPONSE_DATA}},
    http_method: {{HTTP_METHOD}},
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<{{FLOW_NAME}}, {{RESOURCE_COMMON_DATA}}, {{FLOW_REQUEST_DATA}}, {{FLOW_RESPONSE_DATA}}>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<{{FLOW_NAME}}, {{RESOURCE_COMMON_DATA}}, {{FLOW_REQUEST_DATA}}, {{FLOW_RESPONSE_DATA}}>,
        ) -> CustomResult<String, errors::IntegrationError> {
            {{URL_CONSTRUCTION}}
        }
    }
);
```

## Flow-Specific Configuration

### Authorize Flow
```
FLOW_NAME: Authorize
RESOURCE_COMMON_DATA: PaymentFlowData
FLOW_REQUEST_DATA: PaymentsAuthorizeData<T>
FLOW_RESPONSE_DATA: PaymentsResponseData
HTTP_METHOD: Post
CONTENT_TYPE: Json
REQUEST_TYPE: {{CONNECTOR_NAME}}PaymentRequest<T>
RESPONSE_TYPE: {{CONNECTOR_NAME}}PaymentResponse
URL_CONSTRUCTION: Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
```

### PSync Flow
```
FLOW_NAME: PSync
RESOURCE_COMMON_DATA: PaymentFlowData
FLOW_REQUEST_DATA: PaymentsSyncData
FLOW_RESPONSE_DATA: PaymentsResponseData
HTTP_METHOD: Get or Post (depends on connector)
CONTENT_TYPE: Json (if POST) or omit (if GET)
REQUEST_TYPE: {{CONNECTOR_NAME}}SyncRequest or omit (if GET)
RESPONSE_TYPE: {{CONNECTOR_NAME}}SyncResponse
URL_CONSTRUCTION: Ok(format!("{}/v1/payments/{}", self.connector_base_url_payments(req), req.request.connector_transaction_id))
```

### Capture Flow
```
FLOW_NAME: Capture
RESOURCE_COMMON_DATA: PaymentFlowData
FLOW_REQUEST_DATA: PaymentsCaptureData
FLOW_RESPONSE_DATA: PaymentsResponseData
HTTP_METHOD: Post
CONTENT_TYPE: Json
REQUEST_TYPE: {{CONNECTOR_NAME}}CaptureRequest
RESPONSE_TYPE: {{CONNECTOR_NAME}}CaptureResponse
URL_CONSTRUCTION: Ok(format!("{}/v1/payments/{}/capture", self.connector_base_url_payments(req), req.request.connector_transaction_id))
```

### Void Flow
```
FLOW_NAME: Void
RESOURCE_COMMON_DATA: PaymentFlowData
FLOW_REQUEST_DATA: PaymentVoidData
FLOW_RESPONSE_DATA: PaymentsResponseData
HTTP_METHOD: Post or Delete
CONTENT_TYPE: Json
REQUEST_TYPE: {{CONNECTOR_NAME}}VoidRequest
RESPONSE_TYPE: {{CONNECTOR_NAME}}VoidResponse
URL_CONSTRUCTION: Ok(format!("{}/v1/payments/{}/cancel", self.connector_base_url_payments(req), req.request.connector_transaction_id))
```

### Refund Flow
```
FLOW_NAME: Refund
RESOURCE_COMMON_DATA: RefundFlowData
FLOW_REQUEST_DATA: RefundsData
FLOW_RESPONSE_DATA: RefundsResponseData
HTTP_METHOD: Post
CONTENT_TYPE: Json
REQUEST_TYPE: {{CONNECTOR_NAME}}RefundRequest
RESPONSE_TYPE: {{CONNECTOR_NAME}}RefundResponse
URL_CONSTRUCTION: Ok(format!("{}/v1/refunds", self.connector_base_url_refunds(req)))
```

### RSync Flow
```
FLOW_NAME: RSync
RESOURCE_COMMON_DATA: RefundFlowData
FLOW_REQUEST_DATA: RefundSyncData
FLOW_RESPONSE_DATA: RefundsResponseData
HTTP_METHOD: Get
CONTENT_TYPE: Omit (GET request)
REQUEST_TYPE: Omit (GET request)
RESPONSE_TYPE: {{CONNECTOR_NAME}}RefundResponse
URL_CONSTRUCTION: Ok(format!("{}/v1/refunds/{}", self.connector_base_url_refunds(req), req.request.connector_refund_id))
```

## Generation Logic

### Step 1: Parse Tech Spec
Extract from technical specification:
- Connector name
- Supported flows
- API endpoints for each flow
- Request/response formats
- Authentication method
- Amount format (minor/major units)

### Step 2: Generate Flow Definitions
For each supported flow:
1. Determine if flow needs request body (POST/PUT vs GET)
2. Select appropriate `RESOURCE_COMMON_DATA` based on flow type
3. Map to domain_types request/response data types
4. Generate flow definition for `create_all_prerequisites!`

### Step 3: Generate Flow Implementations
For each flow:
1. Create request/response struct names
2. Determine HTTP method from tech spec
3. Extract URL pattern from API documentation
4. Generate `macro_connector_implementation!` block
5. Add custom `get_url` logic based on endpoint

### Step 4: Complete Connector File
1. Add all necessary imports
2. Add trait implementations
3. Insert `create_all_prerequisites!` with all flows
4. Insert `ConnectorCommon` implementation
5. Insert all `macro_connector_implementation!` blocks

## Example: Complete Generation

### Input (Tech Spec):
```yaml
connector_name: ExamplePay
base_url: https://api.examplepay.com
flows:
  - name: authorize
    method: POST
    endpoint: /v1/payments
    request_body: required
  - name: capture
    method: POST
    endpoint: /v1/payments/{id}/capture
    request_body: required
  - name: refund
    method: POST
    endpoint: /v1/refunds
    request_body: required
```

### Output (Generated Code):
```rust
// ... imports ...

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
        (
            flow: Capture,
            request_body: ExamplePayCaptureRequest,
            response_body: ExamplePayCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: ExamplePayRefundRequest,
            response_body: ExamplePayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
    ],
    amount_converters: [
        amount_converter: StringMinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
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

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.examplepay.base_url
        }
    }
);

// ... ConnectorCommon implementation ...

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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ExamplePay,
    curl_request: Json(ExamplePayCaptureRequest),
    curl_response: ExamplePayCaptureResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/v1/payments/{}/capture", self.connector_base_url_payments(req), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ExamplePay,
    curl_request: Json(ExamplePayRefundRequest),
    curl_response: ExamplePayRefundResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}/v1/refunds", self.connector_base_url_refunds(req)))
        }
    }
);
```

## Validation Checklist

After generating code, validate:

- [ ] All flows defined in `create_all_prerequisites!` have corresponding `macro_connector_implementation!`
- [ ] Request types match between flow definition and implementation
- [ ] Response types match between flow definition and implementation
- [ ] `RESOURCE_COMMON_DATA` is correct for each flow type
- [ ] Generic types `<T>` used where payment method data is needed
- [ ] HTTP methods match API specification
- [ ] URL construction uses correct base URL method
- [ ] All imports are present
- [ ] Trait implementations added for all flows
- [ ] `ConnectorCommon` implementation complete
- [ ] Error response parsing implemented
