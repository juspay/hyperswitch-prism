# Fraud Connector Implementation Guide

## Overview

This guide provides detailed instructions for implementing fraud detection connectors in Hyperswitch Prism. **This follows the exact same pattern as PaymentService** - no new patterns are introduced.

## Prerequisites

- Understanding of Rust trait system
- Familiarity with gRPC and Protocol Buffers
- Knowledge of the fraud detection provider's API
- Review of the payouts folder structure in `domain_types/src/payouts/`

## Key Architecture Principles

### 1. NO Separate Trait File in `interfaces` Crate

**PaymentService Pattern**: There is NO `payment.rs` in `interfaces/src/`. Similarly, there is NO `fraud.rs` in `interfaces/src/`.

Instead, connectors implement `ConnectorIntegrationV2` directly in their connector files.

### 2. Flow Markers Live in `connector_flow.rs`

Following the existing pattern:
- `Authorize`, `PSync`, `Void`, etc. are in `domain_types/src/connector_flow.rs`
- Fraud flow markers (`FraudEvaluatePreAuthorization`, etc.) are also in `connector_flow.rs`

### 3. Domain Types Live in `fraud/` Subdirectory

Following the payouts pattern:
- `payouts/` contains `payouts_types.rs`, `types.rs`, `router_request_types.rs`
- `fraud/` contains `fraud_types.rs`, `types.rs`, `router_request_types.rs`

## Connector Structure

### Required Files

1. **Connector Implementation**: `crates/integrations/connector-integration/src/connectors/{connector_name}.rs`
2. **Test Scenarios**: `crates/internal/ucs-connector-tests/scenarios/fraud/{connector_name}/`

### NO Transformers Subdirectory Required

Unlike some payment connectors, fraud connectors typically don't need a separate `transformers.rs` file. Keep implementations simple and self-contained.

## Implementation Steps

### Step 1: Implement `ConnectorCommon`

Every fraud connector must implement the `ConnectorCommon` trait:

```rust
use interfaces::api::ConnectorCommon;

pub struct MyFraudConnector;

impl ConnectorCommon for MyFraudConnector {
    fn id(&self) -> &'static str {
        "my_connector"  // Lowercase, unique identifier
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor  // or CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.my_connector.base_url.as_str()
    }

    fn build_error_response(
        &self,
        res: Response,
        _event_builder: Option<&mut Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Parse connector-specific error format
        // Return standardized ErrorResponse
    }
}
```

### Step 2: Implement Each Fraud Flow

For each of the 6 fraud flows, implement `ConnectorIntegrationV2` directly:

```rust
use interfaces::connector_integration_v2::ConnectorIntegrationV2;
use domain_types::{
    connector_flow,
    fraud::fraud_types::*,
};

impl ConnectorIntegrationV2<
    connector_flow::FraudEvaluatePreAuthorization,
    FraudFlowData,
    FraudEvaluatePreAuthorizationRequest,
    FraudEvaluatePreAuthorizationResponse,
> for MyFraudConnector {
    fn get_headers(
        &self,
        req: &RouterDataV2<..., FraudEvaluatePreAuthorizationRequest, FraudEvaluatePreAuthorizationResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
        // Build HTTP headers
    }

    fn get_url(
        &self,
        req: &RouterDataV2<..., FraudEvaluatePreAuthorizationRequest, FraudEvaluatePreAuthorizationResponse>,
    ) -> CustomResult<String, ConnectorError> {
        // Construct endpoint URL
    }

    fn build_request(
        &self,
        req: &RouterDataV2<..., FraudEvaluatePreAuthorizationRequest, FraudEvaluatePreAuthorizationResponse>,
    ) -> CustomResult<Option<Request>, ConnectorError> {
        // Transform request to connector-specific format
    }

    fn handle_response(
        &self,
        data: &RouterDataV2<..., FraudEvaluatePreAuthorizationRequest, FraudEvaluatePreAuthorizationResponse>,
        res: Response,
    ) -> CustomResult<FraudEvaluatePreAuthorizationResponse, ConnectorError> {
        // Parse connector response
    }
}
```

Repeat for all 6 flows:
1. `FraudEvaluatePreAuthorization`
2. `FraudEvaluatePostAuthorization`
3. `FraudRecordTransactionData`
4. `FraudRecordFulfillmentData`
5. `FraudRecordReturnData`
6. `FraudGet`

**Note**: There is NO `FraudCancel` flow. Cancel is handled via webhooks or status updates.

### Step 3: Register Connector

Add to `crates/integrations/connector-integration/src/connectors.rs`:

```rust
pub mod signifyd;
pub mod riskified;

// In the connector registry function if needed
```

## Data Transformation Patterns

### Request Transformation

Transform internal types directly in the connector implementation:

```rust
fn build_request(
    &self,
    req: &RouterDataV2<..., FraudEvaluatePreAuthorizationRequest, ...>,
) -> CustomResult<Option<Request>, ConnectorError> {
    let connector_request = MyConnectorPreAuthRequest {
        amount: req.request.amount as f64 / 100.0,
        currency: req.request.currency.to_string(),
        device_fingerprint: req.request.device_fingerprint.clone(),
        session_id: req.request.session_id.clone(),
        // ... other fields
    };
    
    Ok(Some(Request {
        body: Some(RequestContent::Json(Box::new(connector_request))),
        // ...
    }))
}
```

### Response Transformation

```rust
fn handle_response(
    &self,
    _data: &RouterDataV2<..., FraudEvaluatePreAuthorizationRequest, FraudEvaluatePreAuthorizationResponse>,
    res: Response,
) -> CustomResult<FraudEvaluatePreAuthorizationResponse, ConnectorError> {
    let connector_response: MyConnectorPreAuthResponse = res
        .response
        .parse_struct("MyConnectorPreAuthResponse")
        .change_context(ConnectorError::ResponseDeserializationFailed)?;
    
    Ok(FraudEvaluatePreAuthorizationResponse {
        fraud_id: connector_response.check_id,
        status: map_status(connector_response.decision),
        recommended_action: map_action(connector_response.action),
        score: connector_response.risk_score.map(|s| FraudScore {
            score: s,
            risk_level: connector_response.risk_level,
            threshold: None,
        }),
        reasons: connector_response.signals.into_iter().map(|s| FraudReason {
            code: s.code,
            message: s.description,
            description: None,
        }).collect(),
        case_id: connector_response.case_id,
        redirect_url: None,
        connector_metadata: None,
    })
}
```

## Error Handling

```rust
fn build_error_response(
    &self,
    res: Response,
    _event_builder: Option<&mut Event>,
) -> CustomResult<ErrorResponse, ConnectorError> {
    let error_body: MyConnectorError = res
        .response
        .parse_struct("MyConnectorError")
        .change_context(ConnectorError::ResponseDeserializationFailed)?;

    Ok(ErrorResponse {
        status_code: res.status_code,
        code: error_body.error_code,
        message: error_body.message,
        reason: error_body.details,
        attempt_status: Some(AttemptStatus::Failure),
        connector_transaction_id: error_body.transaction_id,
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
    })
}
```

## Webhook Handling

If the fraud provider supports webhooks, implement `IncomingWebhook`:

```rust
use interfaces::webhooks::IncomingWebhook;

impl IncomingWebhook for MyFraudConnector {
    fn get_event_type(
        &self,
        request: RequestDetails,
        _secrets: Option<ConnectorWebhookSecrets>,
        _config: Option<ConnectorSpecificConfig>,
    ) -> Result<EventType, error_stack::Report<ConnectorError>> {
        // Parse webhook payload
        // Determine event type (FRM_APPROVED, FRM_REJECTED, etc.)
    }

    fn process_fraud_webhook(
        &self,
        request: RequestDetails,
        _secrets: Option<ConnectorWebhookSecrets>,
        _config: Option<ConnectorSpecificConfig>,
    ) -> Result<FraudWebhookDetailsResponse, error_stack::Report<ConnectorError>> {
        // Parse fraud-specific webhook
    }
}
```

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pre_auth_request_transformation() {
        let request = FraudEvaluatePreAuthorizationRequest {
            amount: 10000,
            currency: Currency::USD,
            device_fingerprint: "fp_abc123".to_string(),
            session_id: "sess_xyz789".to_string(),
            synchronous: true,
            // ...
        };

        // Test transformation logic
    }

    #[test]
    fn test_status_mapping() {
        assert_eq!(map_status("ACCEPT"), FraudCheckStatus::Legit);
        assert_eq!(map_status("REJECT"), FraudCheckStatus::Fraud);
        assert_eq!(map_status("REVIEW"), FraudCheckStatus::ManualReview);
    }
}
```

### Integration Tests

Create test scenarios in `crates/internal/ucs-connector-tests/scenarios/fraud/my_connector/`:

```yaml
# evaluate_pre_auth_approved.yaml
name: "Fraud Evaluate Pre-Authorization - Approved"
flow: FraudEvaluatePreAuthorization
connector: my_connector
request:
  amount:
    minor_amount: 10000
    currency: USD
  device_fingerprint: "fp_test123"
  session_id: "sess_test456"
  synchronous: true
expected:
  status: LEGIT
  recommended_action: ACCEPT
```

## Provider-Specific Notes

### Signifyd
- **Authentication**: Team-based API key in header
- **Endpoints**:
  - Pre-Auth: `/v3/checkouts`
  - Post-Auth: `/v3/transactions`
  - Sales: `/v3/sales`
  - Fulfillments: `/v3/fulfillments`
  - Returns: `/v3/returns`
  - Decisions: `/v3/decisions/{orderId}`
- **Required Fields**: `device_fingerprint`, `session_id`

### Riskified
- **Authentication**: HMAC-SHA256 signature
- **Endpoints**:
  - Submit (async): `/api/orders/submit`
  - Decide (sync): `/api/orders/decide`
  - Update: `/api/orders/update`
  - Fulfill: `/api/orders/fulfill`
  - Partial Refund: `/api/orders/partial_refund`
- **Mode Selection**: Use `synchronous` flag (true=decide, false=submit)

## Common Pitfalls

### 1. NO Trait File in interfaces
**Wrong**: Creating `interfaces/src/fraud.rs` with `FraudConnectorTrait`
**Correct**: Implement `ConnectorIntegrationV2` directly in connector files

### 2. Flow Markers Location
**Wrong**: Defining flow markers in `fraud_types.rs`
**Correct**: Define in `connector_flow.rs` (following existing pattern)

### 3. Folder Structure
**Wrong**: Creating `fraud.rs` at `domain_types/src/fraud_types.rs`
**Correct**: Create `fraud/` subdirectory following payouts pattern

### 4. Flow Marker Derives
**Wrong**: `#[derive(Debug, Clone, Copy)]` (Copy is unnecessary)
**Correct**: `#[derive(Debug, Clone)]` (matching existing flow markers)

### 5. Missing Required Fields
**Signifyd**: Always include `device_fingerprint` and `session_id`
**Riskified**: Always include `session_id` and respect `synchronous` flag

### 6. build.rs Update
**Wrong**: Forgetting to add `"proto/fraud.proto"` to build.rs
**Correct**: Update `crates/types-traits/grpc-api-types/build.rs` compilation list

## Resources

- **Payouts Pattern Reference**: `crates/types-traits/domain_types/src/payouts/`
- **Flow Markers Reference**: `crates/types-traits/domain_types/src/connector_flow.rs`
- **Existing Connectors**: `crates/integrations/connector-integration/src/connectors/`
- **Spec Document**: `docs/plans/fraud/01-fraud-interface-specification.md`
- **Implementation Plan**: `docs/plans/fraud/02-implementation-plan.md`
