# SubmitEvidence Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing the SubmitEvidence flow in **ANY** payment connector. These patterns are extracted from successful connector implementations (Adyen, Stripe, etc.) and can be consumed by AI to generate consistent, production-ready SubmitEvidence flow code for any payment gateway.

## 🚀 Quick Start Guide

To implement a new connector using these patterns:

1. **Choose Your Pattern**: Use [Modern Macro-Based Pattern](#modern-macro-based-pattern-recommended) for 95% of connectors
2. **Replace Placeholders**: Follow the [Placeholder Reference Guide](#placeholder-reference-guide)
3. **Select Components**: Choose request format and evidence document structure based on your connector's API
4. **Follow Checklist**: Use the [Integration Checklist](#integration-checklist) to ensure completeness

### Example: Implementing "NewPayment" Connector

```bash
# Replace placeholders:
{ConnectorName} → NewPayment
{connector_name} → new_payment
{api_endpoint} → "disputes/evidence" (your API endpoint)
```

**✅ Result**: Complete, production-ready SubmitEvidence implementation in ~20 minutes

## Table of Contents

1. [Overview](#overview)
2. [Modern Macro-Based Pattern (Recommended)](#modern-macro-based-pattern-recommended)
3. [Legacy Manual Pattern (Reference)](#legacy-manual-pattern-reference)
4. [Request/Response Format Variations](#requestresponse-format-variations)
5. [Error Handling Patterns](#error-handling-patterns)
6. [Testing Patterns](#testing-patterns)
7. [Integration Checklist](#integration-checklist)

## Overview

The SubmitEvidence flow is a dispute management flow that:
1. Receives evidence submission requests for a dispute from the router
2. Transforms them to connector-specific format with supporting documents
3. Sends requests to the payment gateway's dispute endpoint
4. Processes responses and maps dispute statuses
5. Returns standardized responses to the router

### Key Components:
- **Main Connector File**: Implements traits and flow logic
- **Transformers File**: Handles request/response data transformations
- **Evidence Documents**: Supports multiple document types (receipts, shipping docs, etc.)
- **Error Handling**: Processes and maps error responses
- **Status Mapping**: Converts connector dispute statuses to standard statuses

### Flow Data Types

| Component | Type | Description |
|-----------|------|-------------|
| Flow | `SubmitEvidence` | The flow type identifier |
| resource_common_data | `DisputeFlowData` | Common data for dispute flows |
| flow_request | `SubmitEvidenceData` | Request data with evidence documents |
| flow_response | `DisputeResponseData` | Response data with dispute status |

## Modern Macro-Based Pattern (Recommended)

This is the current recommended approach using the macro framework for maximum code reuse and consistency.

### File Structure Template

```
connector-service/crates/integrations/connector-integration/src/connectors/
├── {connector_name}.rs           # Main connector implementation
└── {connector_name}/
    └── transformers.rs           # Data transformation logic
```

### Main Connector File Pattern

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

pub mod transformers;

use common_utils::{errors::CustomResult, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Accept, Authorize, Capture, DefendDispute, PSync, Refund, SubmitEvidence, Void,
    },
    connector_types::{
        AcceptDisputeData, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundsData, RefundsResponseData, SubmitEvidenceData,
    },
    errors::{self, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorAuthType, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    events::connector_api_logs::ConnectorEvent,
};
use serde::Serialize;
use transformers::{
    {ConnectorName}SubmitEvidenceRequest, {ConnectorName}SubmitEvidenceResponse,
    {ConnectorName}ErrorResponse,
    // Add other request/response types as needed
};

use super::macros;
use crate::types::ResponseRouterData;

pub(crate) mod headers {
    // Define headers used by this connector
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// Trait implementations with generic type parameters
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for {ConnectorName}<T>
{
}

// Dispute-related trait implementations
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::AcceptDispute for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::DisputeDefend for {ConnectorName}<T>
{
}

// Set up connector using macros with all framework integrations
macros::create_all_prerequisites!(
    connector_name: {ConnectorName},
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: {ConnectorName}AuthorizeRequest<T>,
            response_body: {ConnectorName}AuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        // ... other payment flows ...
        (
            flow: SubmitEvidence,
            request_body: {ConnectorName}SubmitEvidenceRequest,
            response_body: {ConnectorName}SubmitEvidenceResponse,
            router_data: RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
        ),
    ],
    amount_converters: [
        // SubmitEvidence typically doesn't need amount converters (no monetary amounts)
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_auth_type)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.{connector_name}.base_url
        }

        pub fn connector_base_url_disputes<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, DisputeFlowData, Req, Res>,
        ) -> Option<&'a str> {
            req.resource_common_data.connectors.{connector_name}.dispute_base_url.as_deref()
        }
    }
);

// Implement ConnectorCommon trait
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    ConnectorCommon for {ConnectorName}<T>
{
    fn id(&self) -> &'static str {
        "{connector_name}"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Minor
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

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
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
}

// Implement SubmitEvidence flow using macro framework
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}SubmitEvidenceRequest),
    curl_response: {ConnectorName}SubmitEvidenceResponse,
    flow_name: SubmitEvidence,
    resource_common_data: DisputeFlowData,
    flow_request: SubmitEvidenceData,
    flow_response: DisputeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_disputes(req)
                .ok_or(errors::IntegrationError::FailedToObtainIntegrationUrl)?;
            let dispute_id = &req.request.connector_dispute_id;
            Ok(format!("{base_url}/{api_endpoint}/{dispute_id}/evidence"))
        }
    }
);
```

### Transformers File Pattern

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

use common_utils::{ext_traits::OptionExt, types::MinorUnit};
use domain_types::{
    connector_flow::SubmitEvidence,
    connector_types::{
        DisputeFlowData, DisputeResponseData, SubmitEvidenceData,
    },
    errors::{self, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorAuthType, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

// Authentication Type Definition
#[derive(Debug)]
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

// Request Structure Template
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}SubmitEvidenceRequest {
    pub dispute_id: String,
    pub evidence_documents: Vec<EvidenceDocument>,
    // Add connector-specific fields
    pub merchant_account: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct EvidenceDocument {
    pub document_type: String,
    pub content: Secret<String>,      // Base64 encoded content
    pub content_type: Option<String>, // MIME type (e.g., "application/pdf")
}

// Response Structure Template
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}SubmitEvidenceResponse {
    pub id: String,
    pub status: {ConnectorName}DisputeStatus,
    pub success: Option<bool>,
    pub error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum {ConnectorName}DisputeStatus {
    NeedsResponse,
    UnderReview,
    Won,
    Lost,
    // Add connector-specific statuses
}

// Error Response Structure
#[derive(Debug, Deserialize, Default)]
pub struct {ConnectorName}ErrorResponse {
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_description: Option<String>,
    pub transaction_id: Option<String>,
}

// Helper struct for router data transformation
pub struct {ConnectorName}RouterData<T, U> {
    pub router_data: T,
    pub connector: U,
}

impl<T, U> TryFrom<(T, U)> for {ConnectorName}RouterData<T, U> {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from((router_data, connector): (T, U)) -> Result<Self, Self::Error> {
        Ok(Self {
            router_data,
            connector,
        })
    }
}

// Request Transformation Implementation
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>, T>>
    for {ConnectorName}SubmitEvidenceRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = {ConnectorName}AuthType::try_from(&router_data.connector_auth_type)?;

        // Build evidence documents from SubmitEvidenceData
        let mut evidence_documents = Vec::new();

        // Add shipping documentation if present
        if let Some(shipping_docs) = &router_data.request.shipping_documentation {
            evidence_documents.push(EvidenceDocument {
                document_type: "shipping_documentation".to_string(),
                content: base64_encode(shipping_docs).into(),
                content_type: router_data.request.shipping_documentation_file_type.clone(),
            });
        }

        // Add receipt if present
        if let Some(receipt) = &router_data.request.receipt {
            evidence_documents.push(EvidenceDocument {
                document_type: "receipt".to_string(),
                content: base64_encode(receipt).into(),
                content_type: router_data.request.receipt_file_type.clone(),
            });
        }

        // Add customer communication if present
        if let Some(communication) = &router_data.request.customer_communication {
            evidence_documents.push(EvidenceDocument {
                document_type: "customer_communication".to_string(),
                content: base64_encode(communication).into(),
                content_type: router_data.request.customer_communication_file_type.clone(),
            });
        }

        // Add more document types as needed...

        Ok(Self {
            dispute_id: router_data.request.connector_dispute_id.clone(),
            evidence_documents,
            merchant_account: auth.api_key.peek().to_string().into(),
        })
    }
}

// Helper function to base64 encode document content
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

// Response Transformation Implementation
impl TryFrom<ResponseRouterData<{ConnectorName}SubmitEvidenceResponse, RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>>>
    for RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}SubmitEvidenceResponse, RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Map connector status to standard dispute status
        let dispute_status = match response.status {
            {ConnectorName}DisputeStatus::NeedsResponse => common_enums::DisputeStatus::DisputeOpened,
            {ConnectorName}DisputeStatus::UnderReview => common_enums::DisputeStatus::DisputeChallenged,
            {ConnectorName}DisputeStatus::Won => common_enums::DisputeStatus::DisputeWon,
            {ConnectorName}DisputeStatus::Lost => common_enums::DisputeStatus::DisputeLost,
        };

        // Check if the evidence submission was successful
        let success = response.success.unwrap_or(false);

        if success {
            let dispute_response_data = DisputeResponseData {
                connector_dispute_id: router_data.request.connector_dispute_id.clone(),
                dispute_status,
                connector_dispute_status: Some(format!("{:?}", response.status)),
                status_code: item.http_code,
            };

            Ok(Self {
                resource_common_data: DisputeFlowData {
                    ..router_data.resource_common_data.clone()
                },
                response: Ok(dispute_response_data),
                ..router_data.clone()
            })
        } else {
            let error_message = response.error_message.clone()
                .unwrap_or_else(|| "Evidence submission failed".to_string());

            let error_response = ErrorResponse {
                code: "EVIDENCE_SUBMISSION_FAILED".to_string(),
                message: error_message.clone(),
                reason: Some(error_message),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(response.id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            };

            Ok(Self {
                resource_common_data: DisputeFlowData {
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error_response),
                ..router_data.clone()
            })
        }
    }
}
```

## Legacy Manual Pattern (Reference)

This pattern shows the older manual implementation style for reference or special cases where macros are insufficient.

### Main Connector File (Manual Implementation)

```rust
#[derive(Clone)]
pub struct {ConnectorName}<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> {ConnectorName}<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

// Manual trait implementation for SubmitEvidence
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for {ConnectorName}<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let mut header = vec![(
            "Content-Type".to_string(),
            "application/json".to_string().into(),
        )];
        let mut api_key = self.get_auth_header(&req.connector_auth_type)?;
        header.append(&mut api_key);
        Ok(header)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
    ) -> CustomResult<String, errors::IntegrationError> {
        let base_url = &req.resource_common_data.connectors.{connector_name}.base_url;
        let dispute_id = &req.request.connector_dispute_id;
        Ok(format!("{base_url}/{endpoint}/{dispute_id}/evidence"))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
    ) -> CustomResult<Option<RequestContent>, errors::IntegrationError> {
        let connector_router_data = {ConnectorName}RouterData::try_from((req.clone(), self))?;
        let connector_req = {ConnectorName}SubmitEvidenceRequest::try_from(&connector_router_data)?;

        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
        event_builder: Option<&mut ConnectorEvent>,
        res: Response,
    ) -> CustomResult<RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>, errors::ConnectorResponseTransformationError> {
        let response: {ConnectorName}SubmitEvidenceResponse = res
            .response
            .parse_struct("{ConnectorName}SubmitEvidenceResponse")
            .change_context(errors::ConnectorResponseTransformationError::ResponseDeserializationFailed { context: Default::default() })?;

        event_builder.map(|i| i.set_response_body(&response));

        RouterDataV2::try_from(ResponseRouterData {
            response,
            data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(errors::ConnectorResponseTransformationError::ResponseHandlingFailed)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorResponseTransformationError> {
        self.build_error_response(res, event_builder)
    }
}
```

## Request/Response Format Variations

### Evidence Document Types

The `SubmitEvidenceData` structure supports various evidence document types:

| Field | Type | Description |
|-------|------|-------------|
| `shipping_documentation` | `Option<Vec<u8>>` | Shipping/tracking documents |
| `receipt` | `Option<Vec<u8>>` | Transaction receipt |
| `customer_communication` | `Option<Vec<u8>>` | Customer emails/messages |
| `customer_signature` | `Option<Vec<u8>>` | Signed documents |
| `cancellation_policy` | `Option<Vec<u8>>` | Cancellation policy document |
| `refund_policy` | `Option<Vec<u8>>` | Refund policy document |
| `service_documentation` | `Option<Vec<u8>>` | Service-related documents |
| `invoice_showing_distinct_transactions` | `Option<Vec<u8>>` | Invoice for distinct transactions |

Each document type has associated metadata fields:
- `{field_name}_file_type` - MIME type (e.g., "application/pdf")
- `{field_name}_provider_file_id` - External file ID if already uploaded

### JSON Format (Most Common)

```rust
// In macro implementation:
curl_request: Json({ConnectorName}SubmitEvidenceRequest),

// Content type:
"Content-Type": "application/json"

// Request structure example:
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}SubmitEvidenceRequest {
    pub dispute_id: String,
    pub documents: Vec<EvidenceDocument>,
}

#[derive(Debug, Serialize)]
pub struct EvidenceDocument {
    #[serde(rename = "type")]
    pub doc_type: String,
    #[serde(with = "base64")]
    pub content: Vec<u8>,
    pub mime_type: String,
}
```

### Multipart Form Data (For Large Files)

```rust
// In macro implementation:
curl_request: FormData({ConnectorName}SubmitEvidenceRequest),

// Content type is set automatically with boundary

// Request structure:
#[derive(Debug, Serialize)]
pub struct {ConnectorName}SubmitEvidenceRequest {
    pub dispute_id: String,
    #[serde(skip)]
    pub files: Vec<UploadFile>,
}

pub struct UploadFile {
    pub field_name: String,
    pub file_name: String,
    pub content_type: String,
    pub content: Vec<u8>,
}
```

## Error Handling Patterns

### Standard Error Response Mapping

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    ConnectorCommon for {ConnectorName}<T>
{
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorResponseTransformationError> {
        let response: {ConnectorName}ErrorResponse = if res.response.is_empty() {
            {ConnectorName}ErrorResponse {
                error_code: Some("HTTP_ERROR".to_string()),
                error_message: Some(format!("HTTP {}", res.status_code)),
                error_description: None,
                transaction_id: None,
            }
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
}
```

### Dispute Status Mapping Pattern

**⚠️ CRITICAL: NEVER HARDCODE STATUS VALUES**

```rust
// CORRECT: Always map from connector response
impl From<{ConnectorName}DisputeStatus> for common_enums::DisputeStatus {
    fn from(status: {ConnectorName}DisputeStatus) -> Self {
        match status {
            {ConnectorName}DisputeStatus::NeedsResponse => Self::DisputeOpened,
            {ConnectorName}DisputeStatus::UnderReview => Self::DisputeChallenged,
            {ConnectorName}DisputeStatus::Won => Self::DisputeWon,
            {ConnectorName}DisputeStatus::Lost => Self::DisputeLost,
            {ConnectorName}DisputeStatus::Accepted => Self::DisputeAccepted,
        }
    }
}
```

### Common Dispute Statuses

| Connector Status | Standard Status | Description |
|------------------|-----------------|-------------|
| `needs_response` | `DisputeOpened` | Dispute opened, awaiting merchant response |
| `under_review` | `DisputeChallenged` | Evidence submitted, under review |
| `won` | `DisputeWon` | Merchant won the dispute |
| `lost` | `DisputeLost` | Merchant lost the dispute |
| `accepted` | `DisputeAccepted` | Merchant accepted the dispute |
| `expired` | `DisputeExpired` | Time limit for response expired |

## Testing Patterns

### Unit Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use domain_types::connector_types::{DisputeFlowData, SubmitEvidenceData};
    use common_enums::DisputeStatus;

    #[test]
    fn test_submit_evidence_request_transformation() {
        // Create test router data
        let router_data = create_test_submit_evidence_router_data();

        // Test request transformation
        let connector_router_data = {ConnectorName}RouterData::try_from(
            (router_data.clone(), &{ConnectorName}::<DefaultPCIHolder>::new())
        ).unwrap();

        let connector_req = {ConnectorName}SubmitEvidenceRequest::try_from(&connector_router_data);

        assert!(connector_req.is_ok());
        let req = connector_req.unwrap();
        assert_eq!(req.dispute_id, "test_dispute_id");
        assert!(!req.evidence_documents.is_empty());
    }

    #[test]
    fn test_submit_evidence_response_transformation() {
        // Test response transformation
        let response = {ConnectorName}SubmitEvidenceResponse {
            id: "test_dispute_id".to_string(),
            status: {ConnectorName}DisputeStatus::UnderReview,
            success: Some(true),
            error_message: None,
        };

        let router_data = create_test_submit_evidence_router_data();
        let response_router_data = ResponseRouterData {
            response,
            data: router_data,
            http_code: 200,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());

        let router_data_result = result.unwrap();
        match &router_data_result.response {
            Ok(data) => assert_eq!(data.dispute_status, DisputeStatus::DisputeChallenged),
            Err(_) => panic!("Expected success response"),
        }
    }

    fn create_test_submit_evidence_router_data() -> RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData> {
        // Create test router data structure
        let submit_evidence_data = SubmitEvidenceData {
            dispute_id: Some("test_dispute_id".to_string()),
            connector_dispute_id: "test_dispute_id".to_string(),
            integrity_object: None,
            access_activity_log: None,
            billing_address: None,
            cancellation_policy: None,
            cancellation_policy_file_type: None,
            cancellation_policy_provider_file_id: None,
            cancellation_policy_disclosure: None,
            cancellation_rebuttal: None,
            customer_communication: Some(b"Test communication".to_vec()),
            customer_communication_file_type: Some("text/plain".to_string()),
            customer_communication_provider_file_id: None,
            customer_email_address: None,
            customer_name: None,
            customer_purchase_ip: None,
            customer_signature: None,
            customer_signature_file_type: None,
            customer_signature_provider_file_id: None,
            product_description: None,
            receipt: Some(b"Test receipt".to_vec()),
            receipt_file_type: Some("application/pdf".to_string()),
            receipt_provider_file_id: None,
            refund_policy: None,
            refund_policy_file_type: None,
            refund_policy_provider_file_id: None,
            refund_policy_disclosure: None,
            refund_refusal_explanation: None,
            service_date: None,
            service_documentation: None,
            service_documentation_file_type: None,
            service_documentation_provider_file_id: None,
            shipping_address: None,
            shipping_carrier: None,
            shipping_date: None,
            shipping_documentation: Some(b"Test shipping doc".to_vec()),
            shipping_documentation_file_type: Some("application/pdf".to_string()),
            shipping_documentation_provider_file_id: None,
            shipping_tracking_number: None,
            invoice_showing_distinct_transactions: None,
            invoice_showing_distinct_transactions_file_type: None,
            invoice_showing_distinct_transactions_provider_file_id: None,
        };

        // Return RouterDataV2 with test data
        // ... implementation
    }
}
```

## Integration Checklist

### Pre-Implementation Checklist

- [ ] **API Documentation Review**
  - [ ] Understand connector's dispute API endpoints
  - [ ] Review evidence submission requirements
  - [ ] Identify supported document types
  - [ ] Understand file size/format limitations
  - [ ] Review error response formats

- [ ] **Integration Requirements**
  - [ ] Determine authentication type
  - [ ] Choose request format (JSON, FormData, etc.)
  - [ ] Identify dispute base URL (may differ from payments URL)

### Implementation Checklist

- [ ] **File Structure Setup**
  - [ ] Create main connector file: `{connector_name}.rs`
  - [ ] Create transformers directory: `{connector_name}/`
  - [ ] Create transformers file: `{connector_name}/transformers.rs`

- [ ] **Main Connector Implementation**
  - [ ] Add `SubmitEvidenceV2` trait implementation
  - [ ] Set up `macros::create_all_prerequisites!` with SubmitEvidence flow
  - [ ] Add dispute base URL accessor in `member_functions`
  - [ ] Implement SubmitEvidence flow with `macros::macro_connector_implementation!`

- [ ] **Transformers Implementation**
  - [ ] Create request structure with evidence documents
  - [ ] Create response structure with dispute status
  - [ ] Implement request transformation
  - [ ] Implement response transformation with status mapping
  - [ ] Add document content encoding (Base64)

### Configuration Checklist

- [ ] **Connector Configuration**
  - [ ] Add `dispute_base_url` to connector config (optional)
  - [ ] Update configuration files (`development.toml`, `production.toml`, `sandbox.toml`)

### Validation Checklist

- [ ] **Code Quality**
  - [ ] Run `cargo build` and fix all errors
  - [ ] Run `cargo test` and ensure all tests pass
  - [ ] Run `cargo clippy` and fix warnings
  - [ ] Run `cargo fmt` for consistent formatting

- [ ] **Functionality Validation**
  - [ ] Test with sandbox/test credentials
  - [ ] Verify evidence submission works correctly
  - [ ] Verify error handling works correctly
  - [ ] Verify status mapping is correct

## Placeholder Reference Guide

**🔄 UNIVERSAL REPLACEMENT SYSTEM**

| Placeholder | Description | Example Values |
|-------------|-------------|----------------|
| `{ConnectorName}` | Connector name in PascalCase | `Stripe`, `Adyen`, `PayPal` |
| `{connector_name}` | Connector name in snake_case | `stripe`, `adyen`, `paypal` |
| `{api_endpoint}` | API endpoint path | `disputes`, `v1/disputes` |

### Real-World Example: Adyen SubmitEvidence

```rust
// Adyen's SubmitEvidence implementation

// Request:
#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenDisputeSubmitEvidenceRequest {
    defense_documents: Vec<DefenseDocuments>,
    merchant_account_code: Secret<String>,
    dispute_psp_reference: String,
}

// Response:
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenSubmitEvidenceResponse {
    pub dispute_service_result: Option<DisputeServiceResult>,
}

// URL Pattern:
// {dispute_base_url}/ca/services/DisputeService/v30/supplyDefenseDocument
```

## Best Practices

### Code Quality and Structure

1. **Use Modern Macro Pattern**: Prefer the macro-based implementation for consistency

2. **Handle All Evidence Types**: Support all relevant evidence document types from `SubmitEvidenceData`

3. **Document Encoding**: Always Base64-encode document content before sending

4. **Status Mapping**: ⚠️ **CRITICAL** - Never hardcode dispute status values
   - **ALWAYS** map status from connector response
   - Use `From` trait or explicit `match` statements

5. **Error Handling**: Provide detailed error messages for document upload failures

### Security Considerations

1. **Document Validation**: Validate document content before encoding/sending
2. **Size Limits**: Check document sizes against connector limits
3. **PII Protection**: Ensure sensitive documents are handled securely

### Critical Reminders

⚠️ **NEVER:**
- Hardcode dispute status values
- Ignore connector-specific document format requirements
- Send documents without proper encoding
- Skip error handling for document upload failures

✅ **ALWAYS:**
- Map status using `From` trait or explicit `match` statements
- Validate document content and size
- Base64-encode binary document content
- Handle partial success scenarios (some documents uploaded, some failed)

---

**Related Patterns:**
- [`pattern_accept_dispute.md`](./pattern_accept_dispute.md) - Accept dispute flow
- [`pattern_defend_dispute.md`](./pattern_defend_dispute.md) - Defend dispute flow
