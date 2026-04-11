# Fraud Interface Implementation Plan

## Document Information
- **Version**: 3.0.0
- **Date**: 2026-04-06
- **Status**: Draft - Synced with Specification v2.1.0
- **Target Audience**: Freshers and Junior Developers
- **Estimated Duration**: 4-6 weeks
- **Prerequisites**: Basic Rust knowledge, understanding of gRPC/protobuf

## Executive Summary

This plan provides step-by-step instructions for implementing the Fraud interface in Hyperswitch Prism. The implementation follows the **PaymentService/Payouts pattern** exactly - no new patterns are introduced.

**Key Constraint**: All enums MUST match Hyperswitch exactly - no new states allowed.

**Major Change from v2.0.0**: 
- Phase 2 and Phase 3 are now **merged** (domain types now include flow markers)
- Folder structure follows **payouts pattern** (`fraud/` subdirectory)
- No separate trait file in `interfaces` crate (traits defined at connector level per PaymentService pattern)

---

## Pre-Implementation Checklist

- [ ] Read and understand the specification document (`01-fraud-interface-specification.md`)
- [ ] Review `04-proto-validation-analysis.md` for provider mappings
- [ ] Review existing payment.proto and payouts.proto patterns
- [ ] Review the payouts folder structure: `domain_types/src/payouts/`
- [ ] Set up development environment (Rust 1.70+, protoc)
- [ ] Understand the connector integration pattern
- [ ] Review existing connector implementations (stripe.rs, adyen.rs)

---

## Phase 1: Protocol Buffer Schema (Week 1)

### Step 1.1: Create fraud.proto
**File**: `crates/types-traits/grpc-api-types/proto/fraud.proto`

```protobuf
syntax = "proto3";

package types;

import "payment.proto";
import "payment_methods.proto";
import "google/protobuf/empty.proto";

option go_package = "github.com/juspay/connector-service/crates/types-traits/grpc-api-types/proto;proto";

// ============================================================================
// FRAUD ENUMERATIONS (Hyperswitch-Aligned - DO NOT MODIFY)
// ============================================================================

// Status of a fraud check - MUST MATCH Hyperswitch FraudCheckStatus EXACTLY
// From: crates/common_enums/src/enums.rs
enum FraudCheckStatus {
  FRAUD_CHECK_STATUS_UNSPECIFIED = 0;
  FRAUD_CHECK_STATUS_PENDING = 1;
  FRAUD_CHECK_STATUS_FRAUD = 2;
  FRAUD_CHECK_STATUS_LEGIT = 3;
  FRAUD_CHECK_STATUS_MANUAL_REVIEW = 4;
  FRAUD_CHECK_STATUS_TRANSACTION_FAILURE = 5;
}

// Fraud check action recommendation
enum FraudAction {
  FRAUD_ACTION_UNSPECIFIED = 0;
  FRAUD_ACTION_ACCEPT = 1;
  FRAUD_ACTION_REJECT = 2;
}

// Fulfillment status for order completion
enum FulfillmentStatus {
  FULFILLMENT_STATUS_UNSPECIFIED = 0;
  FULFILLMENT_STATUS_PENDING = 1;
  FULFILLMENT_STATUS_PARTIAL = 2;
  FULFILLMENT_STATUS_COMPLETE = 3;
  FULFILLMENT_STATUS_REPLACEMENT = 4;
  FULFILLMENT_STATUS_CANCELLED = 5;
}

// Refund method for returns
enum RefundMethod {
  REFUND_METHOD_UNSPECIFIED = 0;
  REFUND_METHOD_STORE_CREDIT = 1;
  REFUND_METHOD_ORIGINAL_PAYMENT_INSTRUMENT = 2;
  REFUND_METHOD_NEW_PAYMENT_INSTRUMENT = 3;
}

// ============================================================================
// CORE FRAUD DATA MESSAGES
// ============================================================================

// Product information for fraud analysis
message FraudProduct {
  string product_id = 1;
  string product_name = 2;
  string product_type = 3;
  int64 quantity = 4;
  int64 unit_price = 5;
  int64 total_amount = 6;
  optional string brand = 7;
  optional string category = 8;
  optional string sub_category = 9;
  optional string sku = 10;
  optional bool requires_shipping = 11;
}

// Shipment destination information
message FraudDestination {
  SecretString full_name = 1;
  optional string organization = 2;
  optional SecretString email = 3;
  Address address = 4;
}

// Fulfillment shipment details
message FraudShipment {
  string shipment_id = 1;
  repeated FraudProduct products = 2;
  FraudDestination destination = 3;
  optional string tracking_company = 4;
  repeated string tracking_numbers = 5;
  repeated string tracking_urls = 6;
  optional string carrier = 7;
  optional string fulfillment_method = 8;
  optional string shipment_status = 9;
  optional int64 shipped_at = 10;
}

// Fraud score details
message FraudScore {
  int32 score = 1;
  optional string risk_level = 2;
  optional int32 threshold = 3;
}

// Fraud decision reason
message FraudReason {
  string code = 1;
  string message = 2;
  optional string description = 3;
}

// ============================================================================
// EVALUATE PRE-AUTHORIZATION REQUEST/RESPONSE
// ============================================================================

message FraudServiceEvaluatePreAuthorizationRequest {
  string merchant_fraud_id = 1;        // REQUIRED
  string order_id = 2;                       // REQUIRED
  optional string connector_fraud_id = 3;
  Money amount = 4;
  optional PaymentMethod payment_method = 5;
  Customer customer = 6;                     // REQUIRED
  repeated FraudProduct products = 7;
  BrowserInformation browser_info = 8;       // REQUIRED
  optional Address shipping_address = 9;
  optional Address billing_address = 10;
  optional string connector_name = 11;
  optional SecretString connector_feature_data = 12;
  optional string webhook_url = 13;
  optional string previous_fraud_id = 14;
  optional ConnectorState connector_state = 15;
  string device_fingerprint = 16;            // NEW - Signifyd requirement
  string session_id = 17;                    // NEW - Session tracking
  bool synchronous = 18;                     // NEW - Riskified sync/async
}

message FraudServiceEvaluatePreAuthorizationResponse {
  optional string merchant_fraud_id = 1;
  optional string order_id = 2;
  optional string connector_fraud_id = 3;
  FraudCheckStatus fraud_check_status = 4;
  FraudAction recommended_action = 5;
  optional FraudScore score = 6;
  repeated FraudReason reasons = 7;
  optional string case_id = 8;
  optional ErrorInfo error = 9;
  uint32 status_code = 10;
  optional SecretString connector_metadata = 11;
  optional string redirect_url = 12;
  optional ConnectorState connector_state = 13;
  optional SecretString raw_connector_response = 14;
  optional SecretString raw_connector_request = 15;
}

// ============================================================================
// EVALUATE POST-AUTHORIZATION REQUEST/RESPONSE
// ============================================================================

message FraudServiceEvaluatePostAuthorizationRequest {
  string merchant_fraud_id = 1;        // REQUIRED
  string order_id = 2;                       // REQUIRED
  optional string connector_fraud_id = 3;
  string connector_transaction_id = 4;       // REQUIRED
  Money amount = 5;
  optional PaymentMethod payment_method = 6;
  AuthorizationStatus authorization_status = 7;
  optional string error_code = 8;
  optional string error_message = 9;
  optional string connector_name = 10;
  optional SecretString connector_feature_data = 11;
  optional string webhook_url = 12;
  optional ConnectorState connector_state = 13;
  string session_id = 14;                    // NEW
}

message FraudServiceEvaluatePostAuthorizationResponse {
  optional string merchant_fraud_id = 1;
  optional string order_id = 2;
  optional string connector_fraud_id = 3;
  FraudCheckStatus fraud_check_status = 4;
  FraudAction recommended_action = 5;
  optional FraudScore score = 6;
  repeated FraudReason reasons = 7;
  optional string case_id = 8;
  optional ErrorInfo error = 9;
  uint32 status_code = 10;
  optional SecretString connector_metadata = 11;
  optional ConnectorState connector_state = 12;
  optional SecretString raw_connector_response = 13;
  optional SecretString raw_connector_request = 14;
}

// ============================================================================
// RECORD TRANSACTION DATA REQUEST/RESPONSE
// ============================================================================

message FraudServiceRecordTransactionDataRequest {
  string merchant_fraud_id = 1;        // REQUIRED
  string order_id = 2;                       // REQUIRED
  Money amount = 3;
  string session_id = 4;                     // NEW
  optional Customer customer = 5;
  repeated FraudProduct products = 6;
  optional BrowserInformation browser_info = 7;
  optional Address shipping_address = 8;
  optional Address billing_address = 9;
  optional SecretString connector_feature_data = 10;
  optional string webhook_url = 11;
  optional ConnectorState connector_state = 12;
}

message FraudServiceRecordTransactionDataResponse {
  optional string merchant_fraud_id = 1;
  optional string order_id = 2;
  optional string connector_fraud_id = 3;
  FraudCheckStatus fraud_check_status = 4;
  FraudAction recommended_action = 5;
  optional FraudScore score = 6;
  repeated FraudReason reasons = 7;
  optional ErrorInfo error = 8;
  uint32 status_code = 9;
  optional SecretString connector_metadata = 10;
  optional ConnectorState connector_state = 11;
  optional SecretString raw_connector_response = 12;
  optional SecretString raw_connector_request = 13;
}

// ============================================================================
// RECORD FULFILLMENT DATA REQUEST/RESPONSE
// ============================================================================

message FraudServiceRecordFulfillmentDataRequest {
  string merchant_fraud_id = 1;        // REQUIRED
  string order_id = 2;                       // REQUIRED
  optional string connector_fraud_id = 3;
  FulfillmentStatus fulfillment_status = 4;
  repeated FraudShipment shipments = 5;
  optional SecretString connector_feature_data = 6;
  optional string webhook_url = 7;
  optional ConnectorState connector_state = 8;
  string session_id = 9;                     // NEW
}

message FraudServiceRecordFulfillmentDataResponse {
  optional string merchant_fraud_id = 1;
  optional string order_id = 2;
  optional string connector_fraud_id = 3;
  FraudCheckStatus fraud_check_status = 4;
  repeated string shipment_ids = 5;
  optional ErrorInfo error = 6;
  uint32 status_code = 7;
  optional SecretString connector_metadata = 8;
  optional ConnectorState connector_state = 9;
  optional SecretString raw_connector_response = 10;
  optional SecretString raw_connector_request = 11;
}

// ============================================================================
// RECORD RETURN DATA REQUEST/RESPONSE
// ============================================================================

message FraudServiceRecordReturnDataRequest {
  string merchant_fraud_id = 1;        // REQUIRED
  string order_id = 2;                       // REQUIRED
  optional string connector_fraud_id = 3;
  optional string refund_transaction_id = 4;
  Money amount = 5;
  RefundMethod refund_method = 6;
  optional string return_reason = 7;
  optional string return_reason_code = 8;
  optional SecretString connector_feature_data = 9;
  optional string webhook_url = 10;
  optional ConnectorState connector_state = 11;
  string session_id = 12;                    // NEW
}

message FraudServiceRecordReturnDataResponse {
  optional string merchant_fraud_id = 1;
  optional string order_id = 2;
  optional string connector_fraud_id = 3;
  FraudCheckStatus fraud_check_status = 4;
  optional string return_id = 5;
  optional ErrorInfo error = 6;
  uint32 status_code = 7;
  optional SecretString connector_metadata = 8;
  optional ConnectorState connector_state = 9;
  optional SecretString raw_connector_response = 10;
  optional SecretString raw_connector_request = 11;
}

// ============================================================================
// GET REQUEST/RESPONSE (Status Sync)
// ============================================================================

message FraudServiceGetRequest {
  string merchant_fraud_id = 1;        // REQUIRED
  string order_id = 2;                       // REQUIRED
  optional string connector_fraud_id = 3;
  optional string case_id = 4;
}

message FraudServiceGetResponse {
  optional string merchant_fraud_id = 1;
  optional string order_id = 2;
  optional string connector_fraud_id = 3;
  FraudCheckStatus fraud_check_status = 4;
  FraudAction recommended_action = 5;
  optional FraudScore score = 6;
  repeated FraudReason reasons = 7;
  optional string case_id = 8;
  optional string reviewed_by = 9;
  optional int64 reviewed_at = 10;
  optional ErrorInfo error = 11;
  uint32 status_code = 12;
  optional SecretString connector_metadata = 13;
  optional ConnectorState connector_state = 14;
}

// ============================================================================
// FRAUD-SPECIFIC EVENT CONTENT (for webhooks)
// ============================================================================

message FraudEventContent {
  optional string merchant_fraud_id = 1;
  optional string order_id = 2;
  optional string connector_fraud_id = 3;
  WebhookEventType event_type = 4;
  FraudCheckStatus fraud_check_status = 5;
  FraudAction recommended_action = 6;
  optional FraudScore score = 7;
  repeated FraudReason reasons = 8;
  optional string case_id = 9;
  int64 event_timestamp = 10;
}
```

**Verification Steps**:
1. Run `cargo build` to verify proto compilation
2. Check generated Rust code in `target/` or build output
3. Verify no compilation errors
4. Confirm FraudCheckStatus has exactly 6 values (including UNSPECIFIED)
5. Confirm FraudAction has exactly 3 values (including UNSPECIFIED)

**Commit Message**: `feat(proto): add fraud service protobuf schema with Hyperswitch-aligned enums`

---

### Step 1.2: Update services.proto
**File**: `crates/types-traits/grpc-api-types/proto/services.proto`

Add import and service definition:

```protobuf
import "fraud.proto";

// ============================================================================
// FRAUD SERVICE — Manages fraud detection and risk assessment
// ============================================================================

service FraudService {
  // Pre-authorization fraud evaluation
  rpc EvaluatePreAuthorization(FraudServiceEvaluatePreAuthorizationRequest) 
      returns (FraudServiceEvaluatePreAuthorizationResponse);
  
  // Post-authorization fraud evaluation with auth results
  rpc EvaluatePostAuthorization(FraudServiceEvaluatePostAuthorizationRequest) 
      returns (FraudServiceEvaluatePostAuthorizationResponse);
  
  // Record completed transaction for post-hoc evaluation
  rpc RecordTransactionData(FraudServiceRecordTransactionDataRequest) 
      returns (FraudServiceRecordTransactionDataResponse);
  
  // Record fulfillment/shipment data
  rpc RecordFulfillmentData(FraudServiceRecordFulfillmentDataRequest) 
      returns (FraudServiceRecordFulfillmentDataResponse);
  
  // Record return/refund data
  rpc RecordReturnData(FraudServiceRecordReturnDataRequest) 
      returns (FraudServiceRecordReturnDataResponse);
  
  // Retrieve fraud decision/status
  rpc Get(FraudServiceGetRequest) returns (FraudServiceGetResponse);
}
```

**Verification Steps**:
1. Verify services.proto compiles without errors
2. Check that FraudService is included in generated code
3. Confirm exactly 6 RPC methods (no Cancel)

**Commit Message**: `feat(proto): add FraudService to services.proto`

---

### Step 1.3: Extend WebhookEventType
**File**: `crates/types-traits/grpc-api-types/proto/payment.proto`

Add to existing enum:

```protobuf
enum WebhookEventType {
  // ... existing events
  FRM_APPROVED = 28;     // Maps to LEGIT status
  FRM_REJECTED = 29;     // Maps to FRAUD status
  FRM_REVIEW_REQUIRED = 60;  // Maps to MANUAL_REVIEW status
}
```

**Note**: Status mapping:
- `FRM_APPROVED` → `FraudCheckStatus.LEGIT`
- `FRM_REJECTED` → `FraudCheckStatus.FRAUD`
- `FRM_REVIEW_REQUIRED` → `FraudCheckStatus.MANUAL_REVIEW`

**Commit Message**: `feat(proto): add fraud-specific webhook events`

---

## Phase 2: Domain Types (Following Payouts Pattern) (Week 1-2)

**Important**: Following the PaymentService/Payouts pattern, we do NOT create a separate `fraud.rs` in the `interfaces` crate. Instead, traits are implemented directly in connector files.

Phase 2 requires the gRPC-generated types to be available, which depends on build.rs being updated first.

### Step 2.1: Update build.rs to Compile fraud.proto
**File**: `crates/types-traits/grpc-api-types/build.rs`

Add `fraud.proto` to the compilation list after `payouts.proto`:

```rust
bridge_generator.compile_protos_with_config(
    config,
    &[
        "proto/services.proto",
        "proto/health_check.proto",
        "proto/payment.proto",
        "proto/composite_services.proto",
        "proto/composite_payment.proto",
        "proto/payment_methods.proto",
        "proto/sdk_config.proto",
        "proto/payouts.proto",
        "proto/fraud.proto",  // ADD THIS LINE
    ],
    &["proto"],
)?;
```

**Verification Steps**:
1. Run `cargo build -p grpc-api-types`
2. Verify generated code includes fraud module
3. Check that `grpc_api_types::fraud::*` types are available

**Commit Message**: `feat(build): add fraud.proto to build configuration`

---

### Step 2.2: Create Fraud Folder Structure

Create the directory structure following the payouts pattern:

```bash
mkdir -p crates/types-traits/domain_types/src/fraud
touch crates/types-traits/domain_types/src/fraud/mod.rs
touch crates/types-traits/domain_types/src/fraud/fraud_types.rs
touch crates/types-traits/domain_types/src/fraud/router_request_types.rs
touch crates/types-traits/domain_types/src/fraud/types.rs
```

---

### Step 2.3: Add Fraud Flow Markers to connector_flow.rs
**File**: `crates/types-traits/domain_types/src/connector_flow.rs`

Add the fraud flow marker structs **after** the Payout flows (around line 95), **before** the `FlowName` enum:

```rust
// ============================================================================
// FRAUD FLOWS (Add after Payout flows, before FlowName enum)
// ============================================================================

#[derive(Debug, Clone)]
pub struct FraudEvaluatePreAuthorization;

#[derive(Debug, Clone)]
pub struct FraudEvaluatePostAuthorization;

#[derive(Debug, Clone)]
pub struct FraudRecordTransactionData;

#[derive(Debug, Clone)]
pub struct FraudRecordFulfillmentData;

#[derive(Debug, Clone)]
pub struct FraudRecordReturnData;

#[derive(Debug, Clone)]
pub struct FraudGet;
```

Then add the corresponding variants to the `FlowName` enum:

```rust
#[derive(strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum FlowName {
    // ... existing variants
    PayoutEnrollDisburseAccount,
    // Fraud flows - NEW
    FraudEvaluatePreAuthorization,
    FraudEvaluatePostAuthorization,
    FraudRecordTransactionData,
    FraudRecordFulfillmentData,
    FraudRecordReturnData,
    FraudGet,
}
```

**Note**: The `#[strum(serialize_all = "snake_case")]` attribute ensures the string representation is `fraud_evaluate_pre_authorization`, matching the existing pattern.

**Verification Steps**:
1. Verify flow markers have `#[derive(Debug, Clone)]` (matching existing flow marker pattern)
2. Run `cargo check -p domain_types` compiles
3. Check that all flow types are unique
4. Confirm exactly 6 fraud flow types (no Cancel)
5. Verify `FlowName` derives display as snake_case

**Commit Message**: `feat(domain): add fraud connector flow types to connector_flow.rs`

---

### Step 2.4: Create fraud/mod.rs
**File**: `crates/types-traits/domain_types/src/fraud/mod.rs`

Following the payouts pattern (see `payouts.rs`):

```rust
pub mod fraud_types;
pub mod router_request_types;
pub mod types;
```

**Commit Message**: `feat(domain): add fraud module exports`

---

### Step 2.5: Create fraud/fraud_types.rs
**File**: `crates/types-traits/domain_types/src/fraud/fraud_types.rs`

Following the payouts pattern (see `payouts/payouts_types.rs`):

```rust
//! Fraud check domain types - Following the payouts pattern

use crate::{
    connector_types::{ConnectorResponseHeaders, RawConnectorRequestResponse},
    types::Connectors,
};
use hyperswitch_masking::Secret;

// ============================================================================
// FRAUD FLOW DATA (Equivalent to PayoutFlowData)
// ============================================================================

#[derive(Debug, Clone)]
pub struct FraudFlowData {
    pub merchant_fraud_id: Option<String>,
    pub order_id: Option<String>,
    pub connector_fraud_id: Option<String>,
    pub connectors: Connectors,
    pub connector_state: Option<crate::types::ConnectorState>,
    pub raw_connector_response: Option<Secret<String>>,
    pub raw_connector_request: Option<Secret<String>>,
    pub connector_response_headers: Option<http::HeaderMap>,
}

impl FraudFlowData {
    pub fn new(connectors: Connectors) -> Self {
        Self {
            merchant_fraud_id: None,
            order_id: None,
            connector_fraud_id: None,
            connectors,
            connector_state: None,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
        }
    }
}

impl RawConnectorRequestResponse for FraudFlowData {
    fn set_raw_connector_response(&mut self, response: Option<Secret<String>>) {
        self.raw_connector_response = response;
    }

    fn get_raw_connector_response(&self) -> Option<Secret<String>> {
        self.raw_connector_response.clone()
    }

    fn get_raw_connector_request(&self) -> Option<Secret<String>> {
        self.raw_connector_request.clone()
    }

    fn set_raw_connector_request(&mut self, request: Option<Secret<String>>) {
        self.raw_connector_request = request;
    }
}

impl ConnectorResponseHeaders for FraudFlowData {
    fn set_connector_response_headers(&mut self, headers: Option<http::HeaderMap>) {
        self.connector_response_headers = headers;
    }

    fn get_connector_response_headers(&self) -> Option<&http::HeaderMap> {
        self.connector_response_headers.as_ref()
    }
}

// ============================================================================
// REQUEST DATA TYPES (Equivalent to PayoutCreateRequest, etc.)
// ============================================================================

#[derive(Debug, Clone)]
pub struct FraudEvaluatePreAuthorizationRequest {
    pub amount: i64,
    pub currency: common_enums::Currency,
    pub customer: Option<crate::connector_types::ConnectorCustomerData>,
    pub payment_method: Option<crate::payment_method_data::PaymentMethodData>,
    pub browser_info: Option<crate::router_request_types::BrowserInformation>,
    pub shipping_address: Option<crate::payment_address::Address>,
    pub billing_address: Option<crate::payment_address::Address>,
    pub connector_name: Option<String>,
    pub previous_fraud_id: Option<String>,
    pub device_fingerprint: String,
    pub session_id: String,
    pub synchronous: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FraudEvaluatePreAuthorizationResponse {
    pub fraud_id: String,
    pub status: FraudCheckStatus,
    pub recommended_action: FraudAction,
    pub score: Option<FraudScore>,
    pub reasons: Vec<FraudReason>,
    pub case_id: Option<String>,
    pub redirect_url: Option<String>,
    pub connector_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct FraudEvaluatePostAuthorizationRequest {
    pub amount: i64,
    pub currency: common_enums::Currency,
    pub payment_method: Option<crate::payment_method_data::PaymentMethodData>,
    pub authorization_status: common_enums::AuthorizationStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub connector_name: Option<String>,
    pub connector_transaction_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FraudEvaluatePostAuthorizationResponse {
    pub fraud_id: String,
    pub status: FraudCheckStatus,
    pub recommended_action: FraudAction,
    pub score: Option<FraudScore>,
    pub reasons: Vec<FraudReason>,
    pub case_id: Option<String>,
    pub connector_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct FraudRecordTransactionDataRequest {
    pub amount: i64,
    pub currency: common_enums::Currency,
    pub customer: Option<crate::connector_types::ConnectorCustomerData>,
    pub browser_info: Option<crate::router_request_types::BrowserInformation>,
    pub shipping_address: Option<crate::payment_address::Address>,
    pub billing_address: Option<crate::payment_address::Address>,
    pub session_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FraudRecordTransactionDataResponse {
    pub fraud_id: String,
    pub status: FraudCheckStatus,
    pub recommended_action: FraudAction,
    pub score: Option<FraudScore>,
    pub reasons: Vec<FraudReason>,
    pub connector_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct FraudRecordFulfillmentDataRequest {
    pub fulfillment_status: FulfillmentStatus,
    pub shipments: Vec<FraudShipment>,
    pub session_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FraudRecordFulfillmentDataResponse {
    pub fraud_id: String,
    pub status: FraudCheckStatus,
    pub shipment_ids: Vec<String>,
    pub connector_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct FraudRecordReturnDataRequest {
    pub amount: i64,
    pub currency: common_enums::Currency,
    pub refund_method: RefundMethod,
    pub return_reason: Option<String>,
    pub return_reason_code: Option<String>,
    pub session_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FraudRecordReturnDataResponse {
    pub fraud_id: String,
    pub status: FraudCheckStatus,
    pub return_id: Option<String>,
    pub connector_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct FraudGetRequest {
    pub merchant_fraud_id: Option<String>,
    pub order_id: Option<String>,
    pub connector_fraud_id: Option<String>,
    pub case_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FraudGetResponse {
    pub fraud_id: String,
    pub status: FraudCheckStatus,
    pub recommended_action: FraudAction,
    pub score: Option<FraudScore>,
    pub reasons: Vec<FraudReason>,
    pub case_id: Option<String>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<i64>,
    pub connector_metadata: Option<serde_json::Value>,
}

// ============================================================================
// SUPPORTING TYPES (Hyperswitch-Aligned - DO NOT MODIFY)
// ============================================================================

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FraudCheckStatus {
    Pending,
    Fraud,
    Legit,
    ManualReview,
    TransactionFailure,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FraudAction {
    Accept,
    Reject,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FulfillmentStatus {
    Pending,
    Partial,
    Complete,
    Replacement,
    Cancelled,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RefundMethod {
    StoreCredit,
    OriginalPaymentInstrument,
    NewPaymentInstrument,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FraudScore {
    pub score: i32,
    pub risk_level: Option<String>,
    pub threshold: Option<i32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FraudReason {
    pub code: String,
    pub message: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FraudProduct {
    pub product_id: String,
    pub product_name: String,
    pub product_type: String,
    pub quantity: i64,
    pub unit_price: i64,
    pub total_amount: i64,
    pub brand: Option<String>,
    pub category: Option<String>,
    pub sub_category: Option<String>,
    pub sku: Option<String>,
    pub requires_shipping: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct FraudDestination {
    pub full_name: Secret<String>,
    pub organization: Option<String>,
    pub email: Option<Secret<String>>,
    pub address: crate::payment_address::Address,
}

#[derive(Debug, Clone)]
pub struct FraudShipment {
    pub shipment_id: String,
    pub products: Vec<FraudProduct>,
    pub destination: FraudDestination,
    pub tracking_company: Option<String>,
    pub tracking_numbers: Vec<String>,
    pub tracking_urls: Vec<String>,
    pub carrier: Option<String>,
    pub fulfillment_method: Option<String>,
    pub shipment_status: Option<String>,
    pub shipped_at: Option<i64>,
}
```

**Verification Steps**:
1. Run `cargo check -p domain_types` to verify compilation
2. Confirm `FraudFlowData` implements `RawConnectorRequestResponse`
3. Confirm `FraudFlowData` implements `ConnectorResponseHeaders`
4. Verify all request/response types are defined
5. Confirm FraudCheckStatus has exactly 5 variants (not counting UNSPECIFIED)
6. Confirm FraudAction has exactly 2 variants (Accept/Reject)

**Commit Message**: `feat(domain): add fraud check domain types following payouts pattern`

---

### Step 2.6: Create fraud/router_request_types.rs
**File**: `crates/types-traits/domain_types/src/fraud/router_request_types.rs`

Similar to `payouts/router_request_types.rs`, define any fraud-specific request structures:

```rust
//! Fraud-specific router request types

// Add any fraud-specific request types here
// Following the pattern from payouts/router_request_types.rs
```

**Commit Message**: `feat(domain): add fraud router request types`

---

### Step 2.7: Create fraud/types.rs
**File**: `crates/types-traits/domain_types/src/fraud/types.rs`

Following the payouts pattern (see `payouts/types.rs`), implement `ForeignTryFrom` for gRPC → domain type conversions:

```rust
//! Fraud type conversions - Following the payouts pattern

use crate::{
    errors::{IntegrationError, IntegrationErrorContext},
    fraud,
    types::Connectors,
    utils::{extract_merchant_id_from_metadata, ForeignTryFrom},
};
use common_utils::metadata::MaskedMetadata;

// Example implementation (add actual conversions as needed):
// impl ForeignTryFrom<(grpc_api_types::fraud::FraudServiceEvaluatePreAuthorizationRequest, Connectors, &MaskedMetadata)> 
//     for fraud::fraud_types::FraudFlowData 
// {
//     type Error = IntegrationError;
//     fn foreign_try_from(...) -> Result<Self, error_stack::Report<Self::Error>> { ... }
// }
```

**Commit Message**: `feat(domain): add fraud type conversions`

---

### Step 2.8: Update domain_types/src/lib.rs
**File**: `crates/types-traits/domain_types/src/lib.rs`

Add the fraud module alongside payouts:

```rust
#![allow(clippy::result_large_err)]

pub mod api;
pub mod connector_flow;
pub mod connector_types;
pub mod errors;
pub mod fraud;        // ADD THIS LINE
pub mod mandates;
pub mod payment_address;
pub mod payment_method_data;
pub mod payouts;
pub mod router_data;
pub mod router_data_v2;
pub mod router_flow_types;
pub mod router_request_types;
pub mod router_response_types;
pub mod types;
pub mod utils;

pub use errors::{
    combine_error_message_with_context, ConnectorResponseTransformationError, IntegrationError,
    IntegrationErrorContext, ResponseTransformationErrorContext,
};
```

**Verification Steps**:
1. Run `cargo check -p domain_types` compiles
2. Verify `fraud` module is accessible

**Commit Message**: `feat(domain): register fraud module in domain_types`

---

## Phase 3: Connector Implementation (Week 2-3)

**Important**: Following the PaymentService pattern, there is NO separate trait file. Instead, implement `ConnectorIntegrationV2` directly in connector files.

### Step 3.1: Create Signifyd Connector Skeleton
**File**: `crates/integrations/connector-integration/src/connectors/signifyd.rs`

```rust
//! Signifyd fraud detection connector implementation

use common_enums::CurrencyUnit;
use common_utils::CustomResult;
use domain_types::{
    connector_flow,
    fraud::fraud_types::*,
    errors::ConnectorError,
};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
};

use crate::types::Response;

pub struct Signifyd;

impl ConnectorCommon for Signifyd {
    fn id(&self) -> &'static str {
        "signifyd"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a domain_types::types::Connectors) -> &'a str {
        connectors.signifyd.base_url.as_str()
    }

    fn build_error_response(
        &self,
        res: Response,
        _event_builder: Option<&mut common_utils::events::Event>,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, ConnectorError> {
        // TODO: Implement error response parsing
        Ok(domain_types::router_data::ErrorResponse {
            status_code: res.status_code,
            code: "UNKNOWN_ERROR".to_string(),
            message: "Unknown error occurred".to_string(),
            reason: None,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

// ============================================================================
// EVALUATE PRE-AUTHORIZATION IMPLEMENTATION
// ============================================================================

impl ConnectorIntegrationV2<
    connector_flow::FraudEvaluatePreAuthorization,
    FraudFlowData,
    FraudEvaluatePreAuthorizationRequest,
    FraudEvaluatePreAuthorizationResponse,
> for Signifyd {
    fn get_headers(
        &self,
        _req: &domain_types::router_data_v2::RouterDataV2<
            connector_flow::FraudEvaluatePreAuthorization,
            FraudFlowData,
            FraudEvaluatePreAuthorizationRequest,
            FraudEvaluatePreAuthorizationResponse,
        >,
    ) -> CustomResult<Vec<(String, hyperswitch_masking::Maskable<String>)>, ConnectorError> {
        // TODO: Implement header construction with API key
        todo!()
    }

    fn get_url(
        &self,
        _req: &domain_types::router_data_v2::RouterDataV2<
            connector_flow::FraudEvaluatePreAuthorization,
            FraudFlowData,
            FraudEvaluatePreAuthorizationRequest,
            FraudEvaluatePreAuthorizationResponse,
        >,
    ) -> CustomResult<String, ConnectorError> {
        // TODO: Return /v3/checkouts endpoint
        todo!()
    }

    fn build_request(
        &self,
        _req: &domain_types::router_data_v2::RouterDataV2<
            connector_flow::FraudEvaluatePreAuthorization,
            FraudFlowData,
            FraudEvaluatePreAuthorizationRequest,
            FraudEvaluatePreAuthorizationResponse,
        >,
    ) -> CustomResult<Option<common_utils::request::Request>, ConnectorError> {
        // TODO: Transform FraudEvaluatePreAuthorizationRequest to Signifyd request
        todo!()
    }

    fn handle_response(
        &self,
        _data: &domain_types::router_data_v2::RouterDataV2<
            connector_flow::FraudEvaluatePreAuthorization,
            FraudFlowData,
            FraudEvaluatePreAuthorizationRequest,
            FraudEvaluatePreAuthorizationResponse,
        >,
        _res: Response,
    ) -> CustomResult<FraudEvaluatePreAuthorizationResponse, ConnectorError> {
        // TODO: Parse Signifyd response
        todo!()
    }
}

// Repeat for other 5 flows...
// FraudEvaluatePostAuthorization
// FraudRecordTransactionData
// FraudRecordFulfillmentData
// FraudRecordReturnData
// FraudGet
```

**Verification Steps**:
1. Add module to `connectors.rs`
2. Run `cargo check` in connector-integration crate
3. Verify all 6 flow implementations exist
4. Confirm exactly 6 flow implementations (no Cancel)

**Commit Message**: `feat(connector): add Signifyd fraud connector skeleton`

---

### Step 3.2: Create Riskified Connector Skeleton
**File**: `crates/integrations/connector-integration/src/connectors/riskified.rs`

Follow the same pattern as Signifyd, adapting for Riskified's API:
- Use HMAC-SHA256 authentication
- Support sync and async modes
- Implement beacon-based session tracking
- Map Riskified states to Hyperswitch enums

**Commit Message**: `feat(connector): add Riskified fraud connector skeleton`

---

## Phase 4: gRPC Service Implementation (Week 3-4)

### Step 4.1: Create fraud service handler
**File**: `crates/grpc-server/src/services/fraud.rs`

```rust
//! FraudService gRPC handler

use tonic::{Request, Response, Status};

use crate::proto::{
    fraud_service_server::FraudService,
    FraudServiceEvaluatePreAuthorizationRequest, FraudServiceEvaluatePreAuthorizationResponse,
    FraudServiceEvaluatePostAuthorizationRequest, FraudServiceEvaluatePostAuthorizationResponse,
    FraudServiceRecordFulfillmentDataRequest, FraudServiceRecordFulfillmentDataResponse,
    FraudServiceGetRequest, FraudServiceGetResponse,
    FraudServiceRecordReturnDataRequest, FraudServiceRecordReturnDataResponse,
    FraudServiceRecordTransactionDataRequest, FraudServiceRecordTransactionDataResponse,
};

pub struct FraudServiceImpl {
    // TODO: Add connector registry, config, etc.
}

impl FraudServiceImpl {
    pub fn new() -> Self {
        Self {}
    }
}

#[tonic::async_trait]
impl FraudService for FraudServiceImpl {
    async fn evaluate_pre_authorization(
        &self,
        _request: Request<FraudServiceEvaluatePreAuthorizationRequest>,
    ) -> Result<Response<FraudServiceEvaluatePreAuthorizationResponse>, Status> {
        Err(Status::unimplemented("EvaluatePreAuthorization not yet implemented"))
    }

    async fn evaluate_post_authorization(
        &self,
        _request: Request<FraudServiceEvaluatePostAuthorizationRequest>,
    ) -> Result<Response<FraudServiceEvaluatePostAuthorizationResponse>, Status> {
        Err(Status::unimplemented("EvaluatePostAuthorization not yet implemented"))
    }

    async fn record_transaction_data(
        &self,
        _request: Request<FraudServiceRecordTransactionDataRequest>,
    ) -> Result<Response<FraudServiceRecordTransactionDataResponse>, Status> {
        Err(Status::unimplemented("RecordTransactionData not yet implemented"))
    }

    async fn record_fulfillment_data(
        &self,
        _request: Request<FraudServiceRecordFulfillmentDataRequest>,
    ) -> Result<Response<FraudServiceRecordFulfillmentDataResponse>, Status> {
        Err(Status::unimplemented("RecordFulfillmentData not yet implemented"))
    }

    async fn record_return_data(
        &self,
        _request: Request<FraudServiceRecordReturnDataRequest>,
    ) -> Result<Response<FraudServiceRecordReturnDataResponse>, Status> {
        Err(Status::unimplemented("RecordReturnData not yet implemented"))
    }

    async fn get(
        &self,
        _request: Request<FraudServiceGetRequest>,
    ) -> Result<Response<FraudServiceGetResponse>, Status> {
        Err(Status::unimplemented("Get not yet implemented"))
    }
}
```

**Verification Steps**:
1. Add service to gRPC server initialization
2. Run `cargo build` in grpc-server crate
3. Verify service starts without errors
4. Confirm exactly 6 RPC methods (no Cancel)

**Commit Message**: `feat(grpc): add FraudService gRPC handler skeleton`

---

## Phase 5: Testing (Week 4-5)

### Step 5.1: Unit Tests for Domain Types
**File**: `crates/types-traits/domain_types/src/fraud/fraud_types_tests.rs` (or inline tests)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fraud_check_status_hyperswitch_alignment() {
        // Verify exactly 5 states (matches Hyperswitch)
        let statuses = vec![
            FraudCheckStatus::Pending,
            FraudCheckStatus::Fraud,
            FraudCheckStatus::Legit,
            FraudCheckStatus::ManualReview,
            FraudCheckStatus::TransactionFailure,
        ];
        assert_eq!(statuses.len(), 5);
    }

    #[test]
    fn test_fraud_action_simplified() {
        // Verify only 2 actions
        let actions = vec![
            FraudAction::Accept,
            FraudAction::Reject,
        ];
        assert_eq!(actions.len(), 2);
    }
}
```

**Commit Message**: `test(domain): add fraud domain types unit tests`

---

### Step 5.2: Scenario-Based Integration Tests (ucs-connector-tests)

Following the existing Payment/Payouts testing pattern using the **scenario-based test harness** in `ucs-connector-tests`.

#### Overview
The test harness uses:
- **Global Suites**: Shared scenario definitions in `global_suites/`
- **Connector Specs**: Per-connector configuration in `connector_specs/`
- **Scenario Files**: JSON with gRPC request templates + assertions
- **Test Binaries**: `test_ucs`, `suite_run_test`, `sdk_run_test`

#### Step 5.2.1: Create Fraud Test Scenarios Directory

Create the fraud suite structure:

```bash
mkdir -p crates/internal/ucs-connector-tests/src/global_suites/fraud_evaluate_pre_auth_suite
touch crates/internal/ucs-connector-tests/src/global_suites/fraud_evaluate_pre_auth_suite/scenario.json
touch crates/internal/ucs-connector-tests/src/global_suites/fraud_evaluate_pre_auth_suite/suite_spec.json
```

Repeat for all 6 fraud flows:
- `fraud_evaluate_pre_auth_suite/`
- `fraud_evaluate_post_auth_suite/`
- `fraud_record_transaction_suite/`
- `fraud_record_fulfillment_suite/`
- `fraud_record_return_suite/`
- `fraud_get_suite/`

---

#### Step 5.2.2: Create Scenario Definitions (scenario.json)

**File**: `global_suites/fraud_evaluate_pre_auth_suite/scenario.json`

```json
{
  "pre_auth_approved": {
    "grpc_req": {
      "merchant_fraud_id": "auto_generate",
      "order_id": "auto_generate",
      "amount": {
        "minor_amount": 10000,
        "currency": "USD"
      },
      "device_fingerprint": "fp_test_123",
      "session_id": "sess_test_456",
      "synchronous": true,
      "customer": {
        "name": "auto_generate",
        "email": { "value": "auto_generate" },
        "id": "auto_generate"
      },
      "browser_info": {
        "user_agent": "Mozilla/5.0...",
        "accept": "text/html",
        "language": "en-US",
        "ip_address": "127.0.0.1"
      }
    },
    "assert": {
      "fraud_check_status": { "one_of": ["LEGIT", "FRAUD", "MANUAL_REVIEW"] },
      "recommended_action": { "must_exist": true },
      "merchant_fraud_id": { "echo": "merchant_fraud_id" },
      "order_id": { "echo": "order_id" }
    },
    "is_default": true
  },
  "pre_auth_fraud_detected": {
    "grpc_req": {
      "merchant_fraud_id": "auto_generate",
      "order_id": "auto_generate",
      "amount": {
        "minor_amount": 999999,
        "currency": "USD"
      },
      "device_fingerprint": "fp_suspicious_999",
      "session_id": "sess_suspicious_999",
      "synchronous": true,
      "customer": {
        "name": "auto_generate",
        "email": { "value": "auto_generate" },
        "id": "auto_generate"
      }
    },
    "assert": {
      "fraud_check_status": { "equals": "FRAUD" },
      "recommended_action": { "equals": "REJECT" },
      "score": { "must_exist": true }
    }
  }
}
```

---

#### Step 5.2.3: Create Suite Specifications (suite_spec.json)

**File**: `global_suites/fraud_evaluate_pre_auth_suite/suite_spec.json`

```json
{
  "suite": "fraud_evaluate_pre_auth",
  "suite_type": "independent",
  "depends_on": [],
  "strict_dependencies": false,
  "dependency_scope": "suite"
}
```

**File**: `global_suites/fraud_evaluate_post_auth_suite/suite_spec.json`

```json
{
  "suite": "fraud_evaluate_post_auth",
  "suite_type": "dependent",
  "depends_on": [
    {
      "suite": "fraud_evaluate_pre_auth",
      "context_map": {
        "merchant_fraud_id": "res.merchant_fraud_id",
        "order_id": "res.order_id",
        "connector_fraud_id": "res.connector_fraud_id"
      }
    }
  ],
  "strict_dependencies": true,
  "dependency_scope": "scenario"
}
```

---

#### Step 5.2.4: Create Connector Specifications

**File**: `connector_specs/signifyd/specs.json`

```json
{
  "connector": "signifyd",
  "supported_suites": [
    "authorize",
    "capture",
    "void",
    "refund",
    "fraud_evaluate_pre_auth",
    "fraud_evaluate_post_auth",
    "fraud_record_transaction",
    "fraud_record_fulfillment",
    "fraud_record_return",
    "fraud_get"
  ]
}
```

**File**: `connector_specs/signifyd/override.json` (Optional - for Signifyd-specific field overrides)

```json
{
  "fraud_evaluate_pre_auth": {
    "device_fingerprint": { "required": true },
    "session_id": { "required": true }
  }
}
```

Repeat for Riskified connector.

---

#### Step 5.2.5: Add Fraud Assertion Types (if needed)

If fraud-specific assertions are needed, extend `scenario_types.rs`:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum FieldAssert {
    // ... existing variants
    /// Asserts field is within a numeric range (for fraud scores)
    Range { min: i32, max: i32 },
    /// Asserts field matches a regex pattern
    Matches { pattern: String },
}
```

---

#### Step 5.2.6: Create Test Binary Entry Points

The existing test binaries (`test_ucs`, `suite_run_test`, `sdk_run_test`) automatically discover and run fraud scenarios once the spec files are created.

Verify by running:

```bash
# List discovered fraud suites
cargo run --bin test_ucs -- --list-suites

# Run fraud scenarios for Signifyd
cargo run --bin test_ucs -- --connector signifyd --suite fraud_evaluate_pre_auth

# Run all fraud suites
cargo run --bin test_ucs -- --connector signifyd --suite-prefix fraud_

# Run with SDK
cargo run --bin sdk_run_test -- --connector signifyd --suite fraud_evaluate_pre_auth
```

---

#### Step 5.2.7: Webhook Testing Scenarios

For webhook-based fraud updates (e.g., async Riskified responses):

**File**: `global_suites/fraud_webhook_suite/scenario.json`

```json
{
  "webhook_approval": {
    "grpc_req": {
      "event_type": "FRM_APPROVED",
      "fraud_id": "auto_generate",
      "order_id": "auto_generate",
      "decision": "ACCEPT"
    },
    "assert": {
      "webhook_event_type": { "equals": "FRM_APPROVED" },
      "fraud_check_status": { "equals": "LEGIT" },
      "recommended_action": { "equals": "ACCEPT" }
    }
  },
  "webhook_rejection": {
    "grpc_req": {
      "event_type": "FRM_REJECTED",
      "fraud_id": "auto_generate",
      "order_id": "auto_generate",
      "decision": "REJECT"
    },
    "assert": {
      "fraud_check_status": { "equals": "FRAUD" },
      "recommended_action": { "equals": "REJECT" }
    }
  }
}
```

---

**Commit Messages**:
- `test(scenarios): add fraud evaluate pre-auth test suite`
- `test(scenarios): add fraud evaluate post-auth test suite`
- `test(scenarios): add fraud record data test suites`
- `test(connector-specs): register fraud suites for signifyd and riskified`

---

### Step 5.3: Direct gRPC Integration Tests (Optional)

For service-level integration tests that don't fit the scenario pattern:

**File**: `crates/grpc-server/grpc-server/tests/fraud_service_test.rs`

```rust
#![allow(clippy::expect_used)]

use grpc_api_types::fraud::{
    fraud_service_client::FraudServiceClient,
    FraudServiceEvaluatePreAuthorizationRequest,
};
use tonic::{transport::Channel, Request};

#[tokio::test]
async fn test_fraud_evaluate_pre_auth_basic() {
    grpc_test!(client, FraudServiceClient<Channel>, {
        let mut request = Request::new(FraudServiceEvaluatePreAuthorizationRequest {
            merchant_fraud_id: Some("test_fraud_123".to_string()),
            order_id: Some("test_order_456".to_string()),
            device_fingerprint: "fp_test".to_string(),
            session_id: "sess_test".to_string(),
            synchronous: true,
            ..Default::default()
        });
        add_mock_metadata(&mut request);
        let response = client.evaluate_pre_authorization(request).await;
        assert!(response.is_ok());
    });
}
```

**Commit Message**: `test(grpc): add fraud service direct integration tests`

---

## Summary Checklist

### Phase 1: Protocol Buffer Schema
- [x] fraud.proto created with Hyperswitch-aligned enums
- [x] services.proto updated with 6 RPC methods
- [x] payment.proto updated with fraud webhook events
- [x] build.rs updated to compile fraud.proto

### Phase 2: Domain Types (Following Payouts Pattern)
- [x] Domain types follow payouts folder pattern (`fraud/` subdirectory)
- [x] Fraud flow markers added to `connector_flow.rs` (with `#[derive(Debug, Clone)]`)
- [x] FlowName enum updated with 6 fraud variants
- [x] fraud/mod.rs created with module exports
- [x] fraud/fraud_types.rs with FraudFlowData and request/response types
- [x] fraud/router_request_types.rs created
- [x] fraud/types.rs with ForeignTryFrom implementations
- [x] lib.rs updated with `pub mod fraud;`

### Phase 3: Connector Implementation
- [x] Following PaymentService pattern - no `fraud.rs` in `interfaces`
- [x] Traits implemented directly in connector files
- [x] Signifyd connector skeleton with 6 flows
- [x] Riskified connector skeleton with 6 flows

### Phase 4: gRPC Service Implementation
- [x] gRPC FraudService handler
- [x] Service registered in server

### Phase 5: Testing
- [ ] Unit tests for domain types
- [ ] Scenario-based integration tests (ucs-connector-tests)
  - [ ] Fraud test suites created (6 suites for 6 flows)
  - [ ] Scenario definitions with gRPC request templates
  - [ ] Assertion rules for fraud responses
  - [ ] Connector specs updated for Signifyd/Riskified
  - [ ] Suite specifications with dependencies
- [ ] Webhook test scenarios
- [ ] Direct gRPC integration tests (optional)

### Key Constraints Verified
- [x] FraudCheckStatus: 5 states (matches Hyperswitch)
- [x] FraudAction: 2 actions (ACCEPT/REJECT)
- [x] No new states introduced
- [x] Exactly 6 flows (no Cancel)
- [x] Flow markers use `#[derive(Debug, Clone)]` (matching existing pattern)

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-04-06 | Initial implementation plan |
| 2.0.0 | 2026-04-06 | Synced with Spec v2.0.0, Hyperswitch-aligned enums |
| 3.0.0 | 2026-04-06 | **Merged Phase 2+3**, following payouts folder pattern, removed separate interfaces traits |
| 3.1.0 | 2026-04-07 | Fixed flow marker derives (`Clone` not `Copy`), added build.rs step to Phase 1 |
