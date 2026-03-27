# Dsync Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing the Dsync (Dispute Sync) flow in **ANY** payment connector within the UCS (Universal Connector Service) system. These patterns are derived from the dispute flow architecture and can be consumed by AI to generate consistent, production-ready Dsync flow code for any payment gateway.

> **🏗️ UCS-Specific:** This pattern is tailored for UCS architecture using RouterDataV2, ConnectorIntegrationV2, and domain_types. Dsync is the dispute equivalent of Psync (Payment Sync) - used to retrieve the current status of a dispute from the connector.

## 🚀 Quick Start Guide

To implement a new connector Dsync flow using these patterns:

1. **Understand Dsync Purpose**: Dsync retrieves dispute status from the connector (like Psync for payments)
2. **Choose Your Pattern**: Use [Modern Macro-Based Pattern](#modern-macro-based-pattern-recommended) for all connectors
3. **Select HTTP Method**: Choose between [GET Pattern](#get-based-dsync-pattern) or [POST Pattern](#post-based-dsync-pattern) based on your API
4. **Replace Placeholders**: Follow the [Placeholder Reference Guide](#placeholder-reference-guide)
5. **Follow Checklist**: Use the [Integration Checklist](#integration-checklist) to ensure completeness

### Example: Implementing "NewPayment" Connector Dsync Flow

```bash
# Replace placeholders:
{ConnectorName} → NewPayment
{connector_name} → new_payment
{HttpMethod} → GET (if API uses RESTful status checking)
{dsync_endpoint} → "v1/disputes/{id}/status" (your dispute status API endpoint)
```

**✅ Result**: Complete, production-ready connector Dsync flow implementation in ~15 minutes

## Table of Contents

1. [Overview](#overview)
2. [Dsync Flow Architecture](#dsync-flow-architecture)
3. [Dsync vs Other Dispute Flows](#dsync-vs-other-dispute-flows)
4. [Modern Macro-Based Pattern (Recommended)](#modern-macro-based-pattern-recommended)
5. [GET-Based Dsync Pattern](#get-based-dsync-pattern)
6. [POST-Based Dsync Pattern](#post-based-dsync-pattern)
7. [URL Construction Patterns](#url-construction-patterns)
8. [Status Mapping Patterns](#status-mapping-patterns)
9. [Error Handling Patterns](#error-handling-patterns)
10. [Testing Patterns](#testing-patterns)
11. [Integration Checklist](#integration-checklist)

## Overview

The Dsync (Dispute Sync) flow is a critical dispute management flow that:
1. Receives dispute status query requests from the router
2. Transforms them to connector-specific query format
3. Sends status requests to the payment gateway using dispute references
4. Processes status responses and maps dispute states
5. Returns standardized dispute status responses to the router

### Key Characteristics:

- **Read-Only Operation**: Dsync only retrieves status, never modifies disputes
- **Idempotent**: Can be called multiple times without side effects
- **Status Synchronization**: Keeps UCS dispute status in sync with connector
- **Polling Support**: Used for polling dispute status updates
- **Webhook Alternative**: Can be used when webhooks are not available or missed

### Key Components:

- **Main Connector File**: Implements Dsync flow logic using macros
- **Transformers File**: Handles Dsync request/response data transformations
- **URL Construction**: Builds status query endpoint URLs (typically with dispute ID)
- **Authentication**: Uses same auth mechanisms as other dispute flows
- **Dispute ID Handling**: Extracts and uses connector dispute references
- **Status Mapping**: Converts connector dispute statuses to standard `DisputeStatus` values

## Dsync Flow Architecture

### Data Flow

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   UCS System    │────▶│  Dsync Request   │────▶│   Connector     │
│                 │     │  (Dispute ID)    │     │   Gateway       │
└─────────────────┘     └──────────────────┘     └─────────────────┘
         │                                               │
         │                                               │
         ▼                                               ▼
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  Update Dispute │◀────│  Dsync Response  │◀────│  Dispute Status │
│  Status in UCS  │     │  (Status Data)   │     │  from Connector │
└─────────────────┘     └──────────────────┘     └─────────────────┘
```

### Flow Relationship

```
Dispute Opened (via Webhook)
      │
      ▼
┌─────────────┐
│   Dsync     │◀── Periodic status checks
│   (Sync)    │
└─────────────┘
      │
      ▼
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Accept    │     │   Defend    │     │   Submit    │
│   (Accept)  │     │(DefendDispute)│    │  Evidence   │
│             │     │             │     │(SubmitEvidence)
└─────────────┘     └─────────────┘     └─────────────┘
```

### Core Types

The Dsync flow uses these core UCS types:

```rust
// From domain_types::connector_flow
#[derive(Debug, Clone)]
pub struct Dsync;

// From domain_types::connector_types
pub struct DisputeFlowData {
    pub dispute_id: Option<String>,
    pub connector_dispute_id: String,
    pub connectors: Connectors,
    pub defense_reason_code: Option<String>,
    pub connector_meta_data: Option<SecretSerdeValue>,
    pub test_mode: Option<bool>,
}

// Note: Dsync uses the same data types as other dispute flows
// The request data is typically minimal - just the dispute ID
pub struct DsyncRequestData {
    pub connector_dispute_id: String,
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
    DisputeAccepted,
    DisputeCancelled,
    DisputeChallenged,
    DisputeWon,
    DisputeLost,
}
```

## Dsync vs Other Dispute Flows

### Critical Differences

| Aspect | Dsync (Sync) | Accept | Defend | SubmitEvidence |
|--------|--------------|--------|--------|----------------|
| **Purpose** | Retrieve status | Accept dispute | Defend dispute | Submit evidence |
| **Read/Write** | Read-only | Write | Write | Write |
| **Idempotent** | Yes | No | No | No |
| **Request Data** | Minimal (ID only) | Minimal | Evidence + docs | Evidence files |
| **Response** | Current status | Confirmation | Confirmation | Confirmation |
| **Frequency** | Multiple times | Once | Once | Multiple times |
| **Outcome** | Status update | Dispute lost | Won/Lost | Evidence recorded |

### When to Use Each Flow

**Use Dsync when:**
- Polling for dispute status updates
- Webhook was missed or failed
- Initial dispute creation needs status confirmation
- Periodic synchronization is required
- Reconciling dispute states

**Use Accept when:**
- Merchant concedes the dispute
- No evidence to defend
- Cost of defense exceeds dispute amount

**Use Defend when:**
- Merchant wants to contest the dispute
- Has compelling evidence
- Dispute amount is significant

**Use SubmitEvidence when:**
- Uploading evidence files
- Providing documentation
- Supporting a defense

## Modern Macro-Based Pattern (Recommended)

This is the current recommended approach using the macro framework for maximum code reuse and consistency.

### Main Connector File Pattern (Dsync Flow Addition)

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

// In the imports section, ensure Dsync flow is included:
use domain_types::{
    connector_flow::{
        Accept, DefendDispute, Dsync, SubmitEvidence, // Add Dsync here
    },
    connector_types::{
        AcceptDisputeData, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        SubmitEvidenceData, DsyncRequestData, // Add DsyncRequestData
    },
};

// In transformers import, include Dsync types:
use transformers::{
    {ConnectorName}DisputeAcceptRequest, {ConnectorName}DisputeAcceptResponse,
    {ConnectorName}DsyncRequest, {ConnectorName}DsyncResponse, // Add Dsync types
    // ... other types
};

// Add Dsync flow to the macro prerequisites
macros::create_all_prerequisites!(
    connector_name: {ConnectorName},
    generic_type: T,
    api: [
        (
            flow: Accept,
            request_body: {ConnectorName}DisputeAcceptRequest,
            response_body: {ConnectorName}DisputeAcceptResponse,
            router_data: RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ),
        (
            flow: Dsync,
            request_body: {ConnectorName}DsyncRequest,
            response_body: {ConnectorName}DsyncResponse,
            router_data: RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>,
        ),
        // Add other dispute flows as needed...
    ],
    amount_converters: [
        // Dsync flows typically don't use amount converters
    ],
    member_functions: {
        // Same build_headers and connector_base_url functions as other flows
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "{content_type}".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_auth_type)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_disputes<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, DisputeFlowData, Req, Res>,
        ) -> Option<&'a str> {
            req.resource_common_data.connectors.{connector_name}.dispute_base_url.as_deref()
        }
    }
);

// Implement Dsync flow using macro framework
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    // Choose appropriate request pattern:
    curl_request: Json({ConnectorName}DsyncRequest), // For POST requests
    // OR
    // (no curl_request line for GET requests)
    curl_response: {ConnectorName}DsyncResponse,
    flow_name: Dsync,
    resource_common_data: DisputeFlowData,
    flow_request: DsyncRequestData,
    flow_response: DisputeResponseData,
    http_method: {HttpMethod}, // Get or Post
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>,
        ) -> CustomResult<String, ConnectorError> {
            // Extract dispute ID from request
            let dispute_id = &req.resource_common_data.connector_dispute_id;

            let base_url = self.connector_base_url_disputes(req)
                .ok_or(errors::ConnectorError::FailedToObtainIntegrationUrl)?;

            // Choose appropriate URL pattern based on connector API:
            // Pattern 1: RESTful with dispute ID in path (most common)
            Ok(format!("{}/{dsync_endpoint}",
                base_url = base_url,
                dsync_endpoint = "{dsync_endpoint}".replace("{id}", dispute_id)
            ))

            // Pattern 2: Query parameter based
            // Ok(format!("{}/disputes?dispute_id={}", base_url, dispute_id))

            // Pattern 3: Dedicated dispute endpoint
            // Ok(format!("{}/disputes/{}/status", base_url, dispute_id))
        }
    }
);

// Add Source Verification stub for Dsync flow
use interfaces::verification::SourceVerification;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    SourceVerification<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>
    for {ConnectorName}<T>
{
    // Stub implementation - will be replaced in Phase 10
}
```

### Transformers File Pattern (Dsync Flow)

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

// Add Dsync-specific imports to existing imports:
use domain_types::{
    connector_flow::{Accept, Dsync}, // Add Dsync here
    connector_types::{
        DisputeFlowData, AcceptDisputeData, DisputeResponseData, DsyncRequestData,
    },
    // ... other imports
};

// Dsync Request Structure (for POST-based connectors)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")] // Adjust based on connector API
pub struct {ConnectorName}DsyncRequest {
    // Common dsync request fields:

    // Dispute reference (required)
    pub dispute_id: String,

    // Merchant information (if required)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_id: Option<String>,

    // Additional fields based on connector requirements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_type: Option<String>,
}

// Alternative: Empty Request Structure (for GET-based connectors)
#[derive(Debug, Serialize)]
pub struct {ConnectorName}DsyncRequest;

// Dsync Response Structure
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")] // Adjust based on connector API
pub struct {ConnectorName}DsyncResponse {
    // Common response fields
    pub id: String,                           // Dispute ID
    pub status: {ConnectorName}DisputeStatus,

    // Reference fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispute_psp_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_psp_reference: Option<String>,

    // Dispute details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_message: Option<String>,

    // Amount information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<{AmountType}>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,

    // Timestamps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,

    // Additional connector-specific fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defense_period_ends_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defense_submitted: Option<bool>,

    // Error information (for failed queries)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

// Dispute Status Enumeration
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")] // Adjust based on connector
pub enum {ConnectorName}DisputeStatus {
    // Common statuses across connectors
    Opened,
    Open,
    Active,

    Accepted,
    Closed,
    Resolved,

    Challenged,
    Defended,
    UnderReview,

    Won,
    Lost,
    Reversed,

    Expired,
    Cancelled,

    // Connector-specific statuses
    Unknown,
}

// Status mapping for Dsync responses
impl From<{ConnectorName}DisputeStatus> for common_enums::DisputeStatus {
    fn from(status: {ConnectorName}DisputeStatus) -> Self {
        match status {
            // Open states
            {ConnectorName}DisputeStatus::Opened
            | {ConnectorName}DisputeStatus::Open
            | {ConnectorName}DisputeStatus::Active => Self::DisputeOpened,

            // Accepted/Closed states
            {ConnectorName}DisputeStatus::Accepted
            | {ConnectorName}DisputeStatus::Closed
            | {ConnectorName}DisputeStatus::Resolved => Self::DisputeAccepted,

            // Challenged states
            {ConnectorName}DisputeStatus::Challenged
            | {ConnectorName}DisputeStatus::Defended
            | {ConnectorName}DisputeStatus::UnderReview => Self::DisputeChallenged,

            // Won states
            {ConnectorName}DisputeStatus::Won
            | {ConnectorName}DisputeStatus::Reversed => Self::DisputeWon,

            // Lost states
            {ConnectorName}DisputeStatus::Lost => Self::DisputeLost,

            // Expired states
            {ConnectorName}DisputeStatus::Expired => Self::DisputeExpired,

            // Cancelled states
            {ConnectorName}DisputeStatus::Cancelled => Self::DisputeCancelled,

            // Unknown/default
            {ConnectorName}DisputeStatus::Unknown => Self::DisputeOpened,
        }
    }
}

// Request Transformation Implementation (for POST-based connectors)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    TryFrom<
        {ConnectorName}RouterData<
            RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>,
            T,
        >,
    > for {ConnectorName}DsyncRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: {ConnectorName}RouterData<
            RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract dispute ID - this is required for sync operations
        let dispute_id = router_data.resource_common_data.connector_dispute_id.clone();

        Ok(Self {
            dispute_id,
            merchant_account: None, // Set if required by connector
            merchant_id: None,      // Set if required by connector
            query_type: Some("dispute_status".to_string()),
        })
    }
}

// Alternative: Empty Request Transformation (for GET-based connectors)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    TryFrom<
        {ConnectorName}RouterData<
            RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>,
            T,
        >,
    > for {ConnectorName}DsyncRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        _item: {ConnectorName}RouterData<
            RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Empty request for GET-based sync
        Ok(Self)
    }
}

// Response Transformation Implementation
impl<F, Req> TryFrom<ResponseRouterData<{ConnectorName}DsyncResponse, Self>>
    for RouterDataV2<F, DisputeFlowData, Req, DisputeResponseData>
{
    type Error = Error;

    fn try_from(
        value: ResponseRouterData<{ConnectorName}DsyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // Map connector status to UCS DisputeStatus
        let status = common_enums::DisputeStatus::from(response.status.clone());

        // Handle error responses
        if let Some(error) = &response.error {
            return Ok(Self {
                resource_common_data: DisputeFlowData {
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: http_code,
                    code: response.error_code.clone().unwrap_or_default(),
                    message: error.clone(),
                    reason: Some(error.clone()),
                    attempt_status: None,
                    connector_transaction_id: Some(response.id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data
            });
        }

        // Success response
        let dispute_response_data = DisputeResponseData {
            connector_dispute_id: response.id.clone(),
            dispute_status: status,
            connector_dispute_status: Some(format!("{:?}", response.status)),
            status_code: http_code,
        };

        Ok(Self {
            resource_common_data: DisputeFlowData {
                ..router_data.resource_common_data.clone()
            },
            response: Ok(dispute_response_data),
            ..router_data
        })
    }
}
```

## GET-Based Dsync Pattern

GET-based Dsync is the most common pattern for dispute status retrieval. It's simple, cacheable, and follows RESTful principles.

### When to Use GET Pattern

- Connector provides RESTful status endpoints
- No sensitive data needs to be sent in request body
- Status checking is idempotent and safe
- URL length limits are not a concern

### GET Pattern Implementation

```rust
// Main connector file - GET implementation
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    // No curl_request for GET - body is empty
    curl_response: {ConnectorName}DsyncResponse,
    flow_name: Dsync,
    resource_common_data: DisputeFlowData,
    flow_request: DsyncRequestData,
    flow_response: DisputeResponseData,
    http_method: Get, // Specify GET method
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
            // GET requests typically don't need Content-Type
            let mut header = vec![];
            let mut auth_header = self.get_auth_header(&req.connector_auth_type)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>,
        ) -> CustomResult<String, ConnectorError> {
            let dispute_id = &req.resource_common_data.connector_dispute_id;
            let base_url = self.connector_base_url_disputes(req)
                .ok_or(errors::ConnectorError::FailedToObtainIntegrationUrl)?;

            // Choose appropriate GET URL pattern:
            // Pattern 1: RESTful with dispute ID in path (most common)
            Ok(format!("{}/disputes/{}", base_url, dispute_id))

            // Pattern 2: Status endpoint pattern
            // Ok(format!("{}/disputes/{}/status", base_url, dispute_id))

            // Pattern 3: Query parameter based
            // Ok(format!("{}/disputes?dispute_id={}", base_url, dispute_id))
        }
    }
);

// Transformers - Empty request structure for GET
#[derive(Debug, Serialize)]
pub struct {ConnectorName}DsyncRequest;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    TryFrom<
        {ConnectorName}RouterData<
            RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>,
            T,
        >,
    > for {ConnectorName}DsyncRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        _item: {ConnectorName}RouterData<
            RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // No request body needed for GET
        Ok(Self)
    }
}
```

## POST-Based Dsync Pattern

POST-based Dsync is used when the API requires complex queries, authentication in the request body, or doesn't support RESTful GET endpoints.

### When to Use POST Pattern

- Connector requires complex query parameters
- Authentication must be in request body
- Sensitive data needs to be sent securely
- API doesn't support RESTful GET endpoints

### POST Pattern Implementation

```rust
// Main connector file - POST implementation
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}DsyncRequest), // Specify JSON request body
    curl_response: {ConnectorName}DsyncResponse,
    flow_name: Dsync,
    resource_common_data: DisputeFlowData,
    flow_request: DsyncRequestData,
    flow_response: DisputeResponseData,
    http_method: Post, // Specify POST method
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_auth_type)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData>,
        ) -> CustomResult<String, ConnectorError> {
            let base_url = self.connector_base_url_disputes(req)
                .ok_or(errors::ConnectorError::FailedToObtainIntegrationUrl)?;

            // Fixed endpoint for POST-based dispute inquiry
            Ok(format!("{}/dispute-inquiry", base_url))
        }
    }
);
```

## URL Construction Patterns

### Pattern 1: RESTful Resource Pattern (Most Common)

```rust
fn get_url(&self, req: &RouterDataV2<Dsync, ...>) -> CustomResult<String, ConnectorError> {
    let dispute_id = &req.resource_common_data.connector_dispute_id;
    let base_url = self.connector_base_url_disputes(req)
        .ok_or(errors::ConnectorError::FailedToObtainIntegrationUrl)?;

    Ok(format!("{}/disputes/{}", base_url, dispute_id))
}
```

### Pattern 2: Status Endpoint Pattern

```rust
fn get_url(&self, req: &RouterDataV2<Dsync, ...>) -> CustomResult<String, ConnectorError> {
    let dispute_id = &req.resource_common_data.connector_dispute_id;
    let base_url = self.connector_base_url_disputes(req)
        .ok_or(errors::ConnectorError::FailedToObtainIntegrationUrl)?;

    Ok(format!("{}/disputes/{}/status", base_url, dispute_id))
}
```

### Pattern 3: Query Parameter Pattern

```rust
fn get_url(&self, req: &RouterDataV2<Dsync, ...>) -> CustomResult<String, ConnectorError> {
    let dispute_id = &req.resource_common_data.connector_dispute_id;
    let base_url = self.connector_base_url_disputes(req)
        .ok_or(errors::ConnectorError::FailedToObtainIntegrationUrl)?;

    Ok(format!("{}/disputes?dispute_id={}", base_url, dispute_id))
}
```

### Pattern 4: Fixed Endpoint Pattern

```rust
fn get_url(&self, req: &RouterDataV2<Dsync, ...>) -> CustomResult<String, ConnectorError> {
    let base_url = self.connector_base_url_disputes(req)
        .ok_or(errors::ConnectorError::FailedToObtainIntegrationUrl)?;

    // Transaction ID goes in request body for POST
    Ok(format!("{}/dispute-inquiry", base_url))
}
```

## Status Mapping Patterns

### Standard Dispute Status Mapping

```rust
// Map connector-specific status to UCS DisputeStatus
impl From<{ConnectorName}DisputeStatus> for common_enums::DisputeStatus {
    fn from(status: {ConnectorName}DisputeStatus) -> Self {
        match status {
            // Open states
            {ConnectorName}DisputeStatus::Opened
            | {ConnectorName}DisputeStatus::Open
            | {ConnectorName}DisputeStatus::Active => Self::DisputeOpened,

            // Accepted/Closed states (merchant conceded)
            {ConnectorName}DisputeStatus::Accepted => Self::DisputeAccepted,
            {ConnectorName}DisputeStatus::Closed |
            {ConnectorName}DisputeStatus::Resolved => {
                // Need to determine if closed won or lost
                // This may require additional context
                Self::DisputeAccepted
            }

            // Challenged states (merchant defending)
            {ConnectorName}DisputeStatus::Challenged
            | {ConnectorName}DisputeStatus::Defended
            | {ConnectorName}DisputeStatus::UnderReview => Self::DisputeChallenged,

            // Won states (merchant won)
            {ConnectorName}DisputeStatus::Won
            | {ConnectorName}DisputeStatus::Reversed => Self::DisputeWon,

            // Lost states (merchant lost)
            {ConnectorName}DisputeStatus::Lost => Self::DisputeLost,

            // Expired states
            {ConnectorName}DisputeStatus::Expired => Self::DisputeExpired,

            // Cancelled states
            {ConnectorName}DisputeStatus::Cancelled => Self::DisputeCancelled,

            // Unknown/default - safest to keep as opened
            _ => Self::DisputeOpened,
        }
    }
}
```

### String-Based Status Mapping

```rust
pub fn map_dispute_status_string(status: &str) -> common_enums::DisputeStatus {
    match status.to_lowercase().as_str() {
        // Open states
        "opened" | "open" | "active" | "new" => common_enums::DisputeStatus::DisputeOpened,

        // Accepted states
        "accepted" | "merchant_conceded" => common_enums::DisputeStatus::DisputeAccepted,

        // Challenged states
        "challenged" | "defended" | "under_review" | "pending_decision" => {
            common_enums::DisputeStatus::DisputeChallenged
        }

        // Won states
        "won" | "reversed" | "merchant_won" | "resolved_in_merchant_favor" => {
            common_enums::DisputeStatus::DisputeWon
        }

        // Lost states
        "lost" | "customer_won" | "resolved_in_customer_favor" => {
            common_enums::DisputeStatus::DisputeLost
        }

        // Expired states
        "expired" | "timed_out" => common_enums::DisputeStatus::DisputeExpired,

        // Cancelled states
        "cancelled" | "withdrawn" => common_enums::DisputeStatus::DisputeCancelled,

        // Default
        _ => common_enums::DisputeStatus::DisputeOpened,
    }
}
```

## Error Handling Patterns

### Dsync-Specific Error Scenarios

1. **Dispute Not Found**: The dispute ID doesn't exist in connector's system
2. **Permission Denied**: Merchant doesn't have access to this dispute
3. **Expired Dispute**: Dispute is too old to retrieve status
4. **Network Timeout**: Connector API is unreachable
5. **Rate Limiting**: Too many sync requests

### Error Response Pattern

```rust
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}DisputeErrorResponse {
    pub error_code: String,
    pub error_message: String,
    pub dispute_status: Option<String>,
}

impl From<{ConnectorName}DisputeErrorResponse> for ErrorResponse {
    fn from(error: {ConnectorName}DisputeErrorResponse) -> Self {
        let attempt_status = match error.error_code.as_str() {
            "DISPUTE_NOT_FOUND" => None,
            "PERMISSION_DENIED" => None,
            "RATE_LIMITED" => None, // Retry later
            _ => None,
        };

        Self {
            status_code: 400,
            code: error.error_code,
            message: error.error_message,
            reason: error.dispute_status,
            attempt_status,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        }
    }
}
```

## Testing Patterns

### Unit Test Structure for Dsync Flow

```rust
#[cfg(test)]
mod dsync_tests {
    use super::*;
    use domain_types::connector_types::DisputeFlowData;

    #[test]
    fn test_dsync_request_transformation_get() {
        // Test GET-based sync request (empty request body)
        let router_data = create_test_dsync_router_data();
        let connector_req = {ConnectorName}DsyncRequest::try_from(router_data);

        assert!(connector_req.is_ok());
    }

    #[test]
    fn test_dsync_request_transformation_post() {
        // Test POST-based sync request transformation
        let router_data = create_test_dsync_router_data();
        let connector_req = {ConnectorName}DsyncRequest::try_from(router_data);

        assert!(connector_req.is_ok());
        let req = connector_req.unwrap();
        assert_eq!(req.dispute_id, "test_dispute_123");
    }

    #[test]
    fn test_dsync_response_transformation_opened() {
        let response = {ConnectorName}DsyncResponse {
            id: "disp_123".to_string(),
            status: {ConnectorName}DisputeStatus::Opened,
            dispute_psp_reference: Some("psp_456".to_string()),
            reason_code: Some("4837".to_string()),
            error: None,
            error_code: None,
        };

        let router_data = create_test_dsync_router_data();
        let response_router_data = ResponseRouterData {
            response,
            router_data,
            http_code: 200,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());

        let router_data_result = result.unwrap();
        assert_eq!(
            router_data_result.response.as_ref().unwrap().dispute_status,
            common_enums::DisputeStatus::DisputeOpened
        );
    }

    #[test]
    fn test_dsync_response_transformation_won() {
        let response = {ConnectorName}DsyncResponse {
            id: "disp_456".to_string(),
            status: {ConnectorName}DisputeStatus::Won,
            dispute_psp_reference: Some("psp_789".to_string()),
            reason_code: None,
            error: None,
            error_code: None,
        };

        let router_data = create_test_dsync_router_data();
        let response_router_data = ResponseRouterData {
            response,
            router_data,
            http_code: 200,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());

        let router_data_result = result.unwrap();
        assert_eq!(
            router_data_result.response.as_ref().unwrap().dispute_status,
            common_enums::DisputeStatus::DisputeWon
        );
    }

    #[test]
    fn test_dsync_status_mapping_all_variants() {
        let test_cases = vec![
            ({ConnectorName}DisputeStatus::Opened, DisputeStatus::DisputeOpened),
            ({ConnectorName}DisputeStatus::Accepted, DisputeStatus::DisputeAccepted),
            ({ConnectorName}DisputeStatus::Challenged, DisputeStatus::DisputeChallenged),
            ({ConnectorName}DisputeStatus::Won, DisputeStatus::DisputeWon),
            ({ConnectorName}DisputeStatus::Lost, DisputeStatus::DisputeLost),
            ({ConnectorName}DisputeStatus::Expired, DisputeStatus::DisputeExpired),
            ({ConnectorName}DisputeStatus::Cancelled, DisputeStatus::DisputeCancelled),
        ];

        for (connector_status, expected_status) in test_cases {
            let mapped_status = DisputeStatus::from(connector_status);
            assert_eq!(mapped_status, expected_status);
        }
    }

    fn create_test_dsync_router_data() -> RouterDataV2<Dsync, DisputeFlowData, DsyncRequestData, DisputeResponseData> {
        RouterDataV2 {
            resource_common_data: DisputeFlowData {
                dispute_id: Some("disp_123".to_string()),
                connector_dispute_id: "test_dispute_123".to_string(),
                connectors: Connectors::default(),
                defense_reason_code: None,
                connector_meta_data: None,
                test_mode: Some(false),
            },
            request: DsyncRequestData {
                connector_dispute_id: "test_dispute_123".to_string(),
            },
            response: Err(ErrorResponse::default()),
            connector_auth_type: ConnectorAuthType::HeaderKey {
                api_key: Secret::new("test_key".to_string()),
            },
        }
    }
}
```

## Integration Checklist

### Pre-Implementation Checklist

- [ ] **API Documentation Review**
  - [ ] Identify dispute status endpoint(s)
  - [ ] Determine HTTP method (GET vs POST)
  - [ ] Understand authentication requirements
  - [ ] Document request/response formats
  - [ ] Identify all possible dispute statuses

- [ ] **Dispute Status Mapping**
  - [ ] Map connector statuses to UCS DisputeStatus enum
  - [ ] Handle ambiguous statuses (e.g., "closed")
  - [ ] Document status transitions

### Implementation Checklist

- [ ] **Main Connector File Updates**
  - [ ] Add `Dsync` to connector_flow imports
  - [ ] Add `DsyncRequestData` to connector_types imports
  - [ ] Import Dsync request/response types from transformers
  - [ ] Add Dsync flow to `macros::create_all_prerequisites!`
  - [ ] Implement Dsync flow with `macros::macro_connector_implementation!`
  - [ ] Choose correct HTTP method (Get or Post)
  - [ ] Add Source Verification stub for Dsync flow

- [ ] **Transformers Implementation**
  - [ ] Add `Dsync` to connector_flow imports
  - [ ] Add `DsyncRequestData` to connector_types imports
  - [ ] Create Dsync request structure
  - [ ] Create Dsync response structure
  - [ ] Create dispute status enumeration
  - [ ] Implement status mapping for Dsync responses
  - [ ] Implement request transformation
  - [ ] Implement response transformation

### Testing Checklist

- [ ] **Unit Tests**
  - [ ] Test request transformation
  - [ ] Test response transformation (all status variants)
  - [ ] Test status mapping
  - [ ] Test error handling

- [ ] **Integration Tests**
  - [ ] Test complete Dsync flow
  - [ ] Test with actual dispute data
  - [ ] Verify status mapping correctness

## Placeholder Reference Guide

| Placeholder | Description | Example Values |
|-------------|-------------|----------------|
| `{ConnectorName}` | Connector name in PascalCase | `Stripe`, `Adyen`, `PayPal` |
| `{connector_name}` | Connector name in snake_case | `stripe`, `adyen`, `paypal` |
| `{HttpMethod}` | HTTP method | `Get`, `Post` |
| `{dsync_endpoint}` | Dsync API endpoint path | `disputes/{id}`, `dispute-inquiry` |
| `{content_type}` | Request content type | `application/json` |
| `{AmountType}` | Amount type | `MinorUnit`, `StringMinorUnit` |

## Related Patterns

- [pattern_accept_dispute.md](./pattern_accept_dispute.md) - Accept dispute flow patterns
- [pattern_defend_dispute.md](./pattern_defend_dispute.md) - Defend dispute flow patterns
- [pattern_submit_evidence.md](./pattern_submit_evidence.md) - Submit evidence flow patterns
- [pattern_psync.md](./pattern_psync.md) - Payment sync (Psync) flow patterns - similar architecture to Dsync

---

**Document Version**: 1.0
**Last Updated**: 2026-02-19
**Compatible with**: UCS Framework v2.x
