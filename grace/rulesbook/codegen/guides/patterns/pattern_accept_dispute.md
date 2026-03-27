# Accept Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing the Accept dispute flow in **ANY** payment connector. These patterns are extracted from successful Accept implementations (Adyen) and can be consumed by AI to generate consistent, production-ready Accept flow code for any payment gateway.

The Accept flow allows merchants to accept a dispute/chargeback, effectively conceding the dispute and allowing the chargeback to proceed without contesting it.

## 🚀 Quick Start Guide

To implement the Accept flow in a new connector:

1. **Understand Accept vs Defend**: Accept concedes the dispute; Defend contests it with evidence
2. **Add Flow Declaration**: Add `Accept` to your connector's macro setup with `DisputeFlowData`
3. **Create Request Structure**: Typically requires dispute ID and merchant credentials
4. **Create Response Structure**: Handle success/failure status from connector
5. **Map Dispute Status**: Success → `DisputeStatus::DisputeAccepted`

### Example: Adding Accept Flow to Existing Connector

```bash
# Add to existing connector:
- Add Accept to macro flow declarations in create_all_prerequisites!
- Create {ConnectorName}DisputeAcceptRequest { dispute_id, merchant_account }
- Create {ConnectorName}DisputeAcceptResponse with success indicator
- Implement URL pattern pointing to dispute accept endpoint
- Map response to DisputeStatus::DisputeAccepted on success
```

**✅ Result**: Working Accept flow integrated into existing connector in ~15 minutes

## Table of Contents

1. [Overview](#overview)
2. [Accept vs Defend Understanding](#accept-vs-defend-understanding)
3. [Accept Flow Architecture](#accept-flow-architecture)
4. [Request/Response Patterns](#requestresponse-patterns)
5. [URL Construction Patterns](#url-construction-patterns)
6. [Status Mapping](#status-mapping)
7. [Error Handling](#error-handling)
8. [Integration with Existing Connectors](#integration-with-existing-connectors)
9. [Testing Strategies](#testing-strategies)
10. [Troubleshooting Guide](#troubleshooting-guide)

## Overview

The Accept flow processes a merchant's decision to accept a dispute (chargeback) rather than contest it. This is the simplest dispute resolution option and is appropriate when:

- The merchant acknowledges the customer's claim is valid
- The cost of contesting exceeds the disputed amount
- There is insufficient evidence to defend the dispute
- The merchant wants to maintain good customer relations

### Key Characteristics:

- **Simple Request Structure**: Usually just dispute reference and merchant identification
- **No Evidence Required**: Unlike SubmitEvidence or DefendDispute flows
- **Final Action**: Accepting a dispute typically ends the dispute process
- **Irreversible**: Most connectors do not allow reversing an accept decision
- **Immediate Effect**: Dispute is typically marked as lost/accepted immediately

### Key Components:

- **Dispute Identification**: Uses `connector_dispute_id` from `DisputeFlowData`
- **Authentication**: Same auth mechanisms as other connector flows
- **Status Mapping**: Maps connector response to `DisputeStatus::DisputeAccepted`
- **Error Handling**: Distinguishes between API errors and business rule violations

## Accept vs Defend Understanding

### Critical Differences

| Aspect | Accept | Defend |
|--------|--------|--------|
| **Purpose** | Concede the dispute | Contest the dispute |
| **Evidence** | None required | Evidence submission required |
| **Outcome** | Dispute lost | May win or lose |
| **Reversible** | Usually no | No |
| **Fees** | Chargeback fees apply | May avoid fees if won |
| **API Complexity** | Simple (just reference) | Complex (evidence, documents) |
| **Status Flow** | Open → Accepted | Open → Challenged → Won/Lost |

### Dispute Lifecycle Context

```
Dispute Opened
      ↓
   [ACCEPT] → Dispute Accepted (Lost)
      ↓
[SUBMIT EVIDENCE] → Evidence Submitted
      ↓
   [DEFEND] → Dispute Challenged
      ↓
   Won / Lost
```

### When to Use Accept vs Defend

**Use Accept when:**
- The customer's claim is legitimate
- You don't have compelling evidence
- The dispute amount is small relative to defense costs
- You want to avoid prolonged dispute process

**Use Defend when:**
- You have strong evidence (delivery proof, T&Cs, etc.)
- The dispute amount is significant
- You believe the chargeback is fraudulent

## Accept Flow Architecture

### Data Flow

1. **Accept Request**: Contains dispute reference, merchant identification
2. **Validation**: Connector validates dispute exists and can be accepted
3. **API Call**: POST to dispute accept endpoint
4. **Dispute Resolution**: Connector marks dispute as accepted/lost
5. **Status Update**: Dispute status updated in UCS system

### Flow Relationship

```
Webhook (Dispute Notification)
      ↓
Dispute Created in UCS
      ↓
Merchant Decision
      ↓
   Accept Flow ←→ Defend Flow
      ↓
Dispute Accepted   Dispute Challenged
```

### Core Types

The Accept flow uses these core UCS types:

```rust
// From domain_types::connector_types

pub struct AcceptDisputeData {
    pub connector_dispute_id: String,
    pub integrity_object: Option<AcceptDisputeIntegrityObject>,
}

pub struct DisputeFlowData {
    pub dispute_id: Option<String>,
    pub connector_dispute_id: String,
    pub connectors: Connectors,
    pub defense_reason_code: Option<String>,
    pub connector_meta_data: Option<SecretSerdeValue>,
    pub test_mode: Option<bool>,
}

pub struct DisputeResponseData {
    pub connector_dispute_id: String,
    pub dispute_status: DisputeStatus,
    pub connector_dispute_status: Option<String>,
    pub status_code: u16,
}

pub enum DisputeStatus {
    DisputeOpened,
    DisputeExpired,
    DisputeAccepted,     // Result of Accept flow
    DisputeCancelled,
    DisputeChallenged,
    DisputeWon,
    DisputeLost,
}
```

## Request/Response Patterns

### Common Request Patterns

#### Pattern 1: Simple Dispute Reference (Most Common)

```rust
// Minimal accept request - dispute ID and merchant account
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}DisputeAcceptRequest {
    pub dispute_id: String,
    pub merchant_account: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        {ConnectorName}RouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    > for {ConnectorName}DisputeAcceptRequest
{
    type Error = Error;

    fn try_from(
        item: {ConnectorName}RouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = {ConnectorName}AuthType::try_from(&item.router_data.connector_auth_type)?;

        Ok(Self {
            dispute_id: item.router_data.resource_common_data.connector_dispute_id.clone(),
            merchant_account: auth.merchant_account.peek().to_string(),
        })
    }
}
```

#### Pattern 2: Service-Specific Request (Adyen-style)

```rust
// For connectors with dedicated dispute service structures
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}DisputeAcceptRequest {
    pub dispute_psp_reference: String,
    pub merchant_account_code: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        {ConnectorName}RouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    > for {ConnectorName}DisputeAcceptRequest
{
    type Error = Error;

    fn try_from(
        item: {ConnectorName}RouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = {ConnectorName}AuthType::try_from(&item.router_data.connector_auth_type)?;

        Ok(Self {
            dispute_psp_reference: item
                .router_data
                .resource_common_data
                .connector_dispute_id
                .clone(),
            merchant_account_code: auth.merchant_account.peek().to_string(),
        })
    }
}
```

#### Pattern 3: Empty Body Request

```rust
// Some connectors accept disputes via URL only
#[derive(Debug, Clone, Serialize)]
pub struct {ConnectorName}DisputeAcceptRequest {}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        {ConnectorName}RouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    > for {ConnectorName}DisputeAcceptRequest
{
    type Error = Error;

    fn try_from(
        _item: {ConnectorName}RouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}
```

### Common Response Patterns

#### Pattern 1: Service Result Response (Adyen-style)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}DisputeAcceptResponse {
    pub dispute_service_result: Option<DisputeServiceResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisputeServiceResult {
    pub success: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

impl<F, Req> TryFrom<ResponseRouterData<{ConnectorName}DisputeAcceptResponse, Self>>
    for RouterDataV2<F, DisputeFlowData, Req, DisputeResponseData>
{
    type Error = Error;

    fn try_from(
        value: ResponseRouterData<{ConnectorName}DisputeAcceptResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let success = response
            .dispute_service_result
            .as_ref()
            .is_some_and(|r| r.success);

        if success {
            let status = common_enums::DisputeStatus::DisputeAccepted;

            let dispute_response_data = DisputeResponseData {
                dispute_status: status,
                connector_dispute_id: router_data
                    .resource_common_data
                    .connector_dispute_id
                    .clone(),
                connector_dispute_status: None,
                status_code: http_code,
            };

            Ok(Self {
                resource_common_data: DisputeFlowData {
                    ..router_data.resource_common_data
                },
                response: Ok(dispute_response_data),
                ..router_data
            })
        } else {
            // Handle error case
            let error = response.dispute_service_result.as_ref().and_then(|r| {
                r.error_message.clone().map(|msg| ErrorResponse {
                    status_code: http_code,
                    code: r.error_code.clone().unwrap_or_default(),
                    message: msg,
                    reason: None,
                    attempt_status: None,
                    connector_transaction_id: None,
                })
            });

            Ok(Self {
                response: Err(error.unwrap_or_else(|| ErrorResponse {
                    status_code: http_code,
                    code: "UNKNOWN_ERROR".to_string(),
                    message: "Unknown error in dispute accept".to_string(),
                    reason: None,
                    attempt_status: None,
                    connector_transaction_id: None,
                })),
                ..router_data
            })
        }
    }
}
```

#### Pattern 2: Simple Status Response

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {ConnectorName}DisputeAcceptResponse {
    pub id: String,
    pub status: String,
    pub accepted_at: Option<String>,
}

impl<F, Req> TryFrom<ResponseRouterData<{ConnectorName}DisputeAcceptResponse, Self>>
    for RouterDataV2<F, DisputeFlowData, Req, DisputeResponseData>
{
    type Error = Error;

    fn try_from(
        value: ResponseRouterData<{ConnectorName}DisputeAcceptResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let status = match response.status.as_str() {
            "accepted" | "closed" => common_enums::DisputeStatus::DisputeAccepted,
            "pending" => common_enums::DisputeStatus::DisputeChallenged,
            _ => common_enums::DisputeStatus::DisputeOpened,
        };

        let dispute_response_data = DisputeResponseData {
            dispute_status: status,
            connector_dispute_id: response.id,
            connector_dispute_status: Some(response.status),
            status_code: http_code,
        };

        Ok(Self {
            resource_common_data: DisputeFlowData {
                ..router_data.resource_common_data
            },
            response: Ok(dispute_response_data),
            ..router_data
        })
    }
}
```

## URL Construction Patterns

### Pattern 1: Dedicated Dispute Base URL

```rust
// For connectors with separate dispute API endpoints
fn get_url(
    &self,
    req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
) -> CustomResult<String, errors::ConnectorError> {
    let dispute_url = self.connector_base_url_disputes(req)
        .ok_or(errors::ConnectorError::FailedToObtainIntegrationUrl)?;
    Ok(format!("{dispute_url}/disputes/{}/accept",
        req.resource_common_data.connector_dispute_id))
}
```

### Pattern 2: Standard Base URL with Dispute Path

```rust
// For connectors using standard base URL with dispute path
fn get_url(
    &self,
    req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
) -> CustomResult<String, errors::ConnectorError> {
    let base_url = self.connector_base_url(req);
    Ok(format!("{}/v1/disputes/{}/accept",
        base_url,
        req.resource_common_data.connector_dispute_id))
}
```

### Pattern 3: Service-Specific Endpoint

```rust
// For connectors with SOAP or specific service endpoints
fn get_url(
    &self,
    req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
) -> CustomResult<String, errors::ConnectorError> {
    let dispute_url = self.connector_base_url_disputes(req)
        .ok_or(errors::ConnectorError::FailedToObtainIntegrationUrl)?;
    Ok(format!("{dispute_url}/services/DisputeService/v30/acceptDispute"))
}
```

## Status Mapping

### Dispute Status Mapping

| Connector Status | UCS DisputeStatus | Meaning |
|------------------|-------------------|---------|
| `accepted`, `closed`, `resolved` | `DisputeAccepted` | Dispute successfully accepted |
| `pending`, `processing` | `DisputeChallenged` | Accept request being processed |
| `failed`, `error` | `DisputeOpened` | Accept failed, dispute remains open |
| `expired` | `DisputeExpired` | Dispute acceptance window expired |

### Standard Mapping Pattern

```rust
let status = match response.status.as_str() {
    // Success cases
    "accepted" | "closed" | "resolved" | "won" =>
        common_enums::DisputeStatus::DisputeAccepted,

    // Processing cases
    "pending" | "processing" | "submitted" =>
        common_enums::DisputeStatus::DisputeChallenged,

    // Error cases - dispute remains open
    "failed" | "error" | "rejected" =>
        common_enums::DisputeStatus::DisputeOpened,

    // Unknown - default to safe state
    _ => common_enums::DisputeStatus::DisputeOpened,
};
```

## Error Handling

### Common Error Scenarios

1. **Dispute Not Found**: The dispute ID doesn't exist
2. **Already Resolved**: Dispute is already accepted, defended, or expired
3. **Invalid State**: Dispute is not in a state that can be accepted
4. **Time Expired**: Acceptance window has passed
5. **Permission Denied**: Merchant doesn't own the dispute

### Error Response Pattern

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {ConnectorName}DisputeErrorResponse {
    pub error_code: String,
    pub error_message: String,
    pub dispute_status: Option<String>,
}

impl From<{ConnectorName}DisputeErrorResponse> for ErrorResponse {
    fn from(error: {ConnectorName}DisputeErrorResponse) -> Self {
        Self {
            status_code: 400,
            code: error.error_code,
            message: error.error_message,
            reason: error.dispute_status,
            attempt_status: None,
            connector_transaction_id: None,
        }
    }
}
```

## Integration with Existing Connectors

### Adding Accept Flow to an Existing Connector

1. **Update Flow Declarations** in `create_all_prerequisites!`:

```rust
macros::create_all_prerequisites!(
    connector_name: {ConnectorName},
    generic_type: T,
    api: [
        // ... existing flows ...
        (
            flow: Accept,
            request_body: {ConnectorName}DisputeAcceptRequest,
            response_body: {ConnectorName}DisputeAcceptResponse,
            router_data: RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ),
        // ... other flows ...
    ],
    amount_converters: [
        // Accept flows typically don't use amount converters
    ],
    member_functions: {
        // Add helper if needed for dispute base URL
        pub fn connector_base_url_disputes<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, DisputeFlowData, Req, Res>,
        ) -> Option<&'a str> {
            req.resource_common_data.connectors.{connector_name}.dispute_base_url.as_deref()
        }
        // ... other functions ...
    }
);
```

2. **Implement the Flow** using `macro_connector_implementation!`:

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}DisputeAcceptRequest),
    curl_response: {ConnectorName}DisputeAcceptResponse,
    flow_name: Accept,
    resource_common_data: DisputeFlowData,
    flow_request: AcceptDisputeData,
    flow_response: DisputeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let base_url = self.connector_base_url(req);
            Ok(format!("{}/disputes/{}/accept",
                base_url,
                req.resource_common_data.connector_dispute_id))
        }
    }
);
```

3. **Add Trait Implementation** (if not already present):

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for {ConnectorName}<T>
{
}
```

### Configuration Requirements

Add to `config/development.toml`, `config/sandbox.toml`, and `config/production.toml`:

```toml
[connectors.{connector_name}]
base_url = "https://api.{connector_name}.com"
dispute_base_url = "https://disputes.{connector_name}.com"  # Optional: if separate
```

## Testing Strategies

### Unit Test Patterns

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispute_accept_request_transform() {
        let router_data = create_test_router_data();
        let result = {ConnectorName}DisputeAcceptRequest::try_from(router_data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().dispute_id, "test_dispute_id");
    }

    #[test]
    fn test_dispute_accept_response_success() {
        let response = {ConnectorName}DisputeAcceptResponse {
            status: "accepted".to_string(),
            id: "disp_123".to_string(),
        };
        // Test transformation...
    }
}
```

### Integration Test Scenarios

1. **Happy Path**: Successfully accept a dispute
2. **Already Accepted**: Handle attempt to accept already-accepted dispute
3. **Invalid Dispute ID**: Handle non-existent dispute
4. **Expired Dispute**: Handle acceptance after deadline
5. **Network Errors**: Handle timeouts and connectivity issues

## Troubleshooting Guide

### Common Issues

#### Issue: "Dispute not found" error

**Cause**: The `connector_dispute_id` doesn't match any dispute in the connector's system.

**Solution**:
- Verify dispute ID is passed correctly from webhook
- Check if dispute ID format matches connector expectations
- Ensure dispute hasn't been deleted/archived

#### Issue: "Dispute cannot be accepted" error

**Cause**: Dispute is in a state that doesn't allow acceptance (already resolved, expired, etc.).

**Solution**:
- Check dispute status before calling accept
- Handle `DisputeExpired` status appropriately
- Inform merchant of alternative actions (e.g., defend if still possible)

#### Issue: Empty response or timeout

**Cause**: Connector API issues or network problems.

**Solution**:
- Implement retry logic for idempotent operations
- Set appropriate timeout values
- Log request/response for debugging

### Debug Checklist

- [ ] Verify `connector_dispute_id` is populated in `DisputeFlowData`
- [ ] Confirm authentication credentials are valid
- [ ] Check connector dispute API URL configuration
- [ ] Verify dispute is in acceptable state before calling
- [ ] Review connector-specific error codes

## Placeholder Reference Guide

| Placeholder | Replace With | Example |
|-------------|--------------|---------|
| `{ConnectorName}` | PascalCase connector name | `Stripe`, `Adyen`, `Checkout` |
| `{connector_name}` | snake_case connector name | `stripe`, `adyen`, `checkout` |
| `{AmountType}` | Amount converter (if applicable) | `StringMinorUnit`, `MinorUnit` |
| `{content_type}` | Content-Type header value | `application/json` |
| `{endpoint}` | API endpoint path | `disputes`, `chargebacks` |
| `{version}` | API version | `v1`, `v30` |

## Integration Checklist

### Pre-Implementation

- [ ] Review connector's dispute API documentation
- [ ] Identify dispute accept endpoint
- [ ] Determine authentication method
- [ ] Understand dispute status lifecycle

### Implementation

- [ ] Add `Accept` flow declaration in `create_all_prerequisites!`
- [ ] Create `{ConnectorName}DisputeAcceptRequest` struct
- [ ] Implement `TryFrom` for request transformation
- [ ] Create `{ConnectorName}DisputeAcceptResponse` struct
- [ ] Implement response transformation with status mapping
- [ ] Add `macro_connector_implementation!` for Accept flow
- [ ] Implement `ConnectorIntegrationV2<Accept, ...>` trait
- [ ] Configure dispute base URL in config files

### Testing

- [ ] Unit test request transformation
- [ ] Unit test response transformation
- [ ] Integration test with sandbox environment
- [ ] Error scenario testing
- [ ] Verify status mapping correctness

### Post-Implementation

- [ ] Update connector documentation
- [ ] Add dispute flow to connector capabilities list
- [ ] Test end-to-end with real dispute scenario
- [ ] Monitor for errors in production

## Related Patterns

- [pattern_defend_dispute.md](./pattern_defend_dispute.md) - Defending disputes with evidence
- [pattern_submit_evidence.md](./pattern_submit_evidence.md) - Submitting evidence for disputes
- [pattern_webhook.md](./pattern_webhook.md) - Handling dispute webhooks

## Example: Complete Adyen Implementation Reference

```rust
// File: crates/integrations/connector-integration/src/connectors/adyen/transformers.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenDisputeAcceptRequest {
    pub dispute_psp_reference: String,
    pub merchant_account_code: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    > for AdyenDisputeAcceptRequest
{
    type Error = Error;

    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = AdyenAuthType::try_from(&item.router_data.connector_auth_type)?;

        Ok(Self {
            dispute_psp_reference: item
                .router_data
                .resource_common_data
                .connector_dispute_id
                .clone(),
            merchant_account_code: auth.merchant_account.peek().to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenDisputeAcceptResponse {
    pub dispute_service_result: Option<DisputeServiceResult>,
}

impl<F, Req> TryFrom<ResponseRouterData<AdyenDisputeAcceptResponse, Self>>
    for RouterDataV2<F, DisputeFlowData, Req, DisputeResponseData>
{
    type Error = Error;

    fn try_from(
        value: ResponseRouterData<AdyenDisputeAcceptResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;
        let success = response
            .dispute_service_result
            .as_ref()
            .is_some_and(|r| r.success);

        if success {
            let status = common_enums::DisputeStatus::DisputeAccepted;

            let dispute_response_data = DisputeResponseData {
                dispute_status: status,
                connector_dispute_id: router_data
                    .resource_common_data
                    .connector_dispute_id
                    .clone(),
                connector_dispute_status: None,
                status_code: http_code,
            };

            Ok(Self {
                resource_common_data: DisputeFlowData {
                    ..router_data.resource_common_data
                },
                response: Ok(dispute_response_data),
                ..router_data
            })
        } else {
            // Error handling...
        }
    }
}
```

```rust
// File: crates/integrations/connector-integration/src/connectors/adyen.rs

// In create_all_prerequisites! macro:
(
    flow: Accept,
    request_body: AdyenDisputeAcceptRequest,
    response_body: AdyenDisputeAcceptResponse,
    router_data: RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
),

// Accept flow implementation:
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenDisputeAcceptRequest),
    curl_response: AdyenDisputeAcceptResponse,
    flow_name: Accept,
    resource_common_data: DisputeFlowData,
    flow_request: AcceptDisputeData,
    flow_response: DisputeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let dispute_url = self.connector_base_url_disputes(req)
                .ok_or(errors::ConnectorError::FailedToObtainIntegrationUrl)?;
            Ok(format!("{dispute_url}/ca/services/DisputeService/v30/acceptDispute"))
        }
    }
);
```

---

**Document Version**: 1.0
**Last Updated**: 2025-01-XX
**Compatible with**: UCS Framework v2.x
