# Fraud Interface Specification for Hyperswitch Prism

## Document Information
- **Version**: 2.1.0
- **Date**: 2026-04-06
- **Status**: Reviewed and Approved
- **Author**: AI Design Agent
- **Reviewer**: Spec Verifier Agent

## 1. Executive Summary

This document specifies the Fraud interface for Hyperswitch Prism, enabling unified integration with fraud detection providers like Signifyd and Riskified. The interface follows the same architectural patterns as Payments and Payouts services, ensuring consistency across the Prism library.

### Key Objectives
1. Provide a unified API for fraud detection across multiple providers
2. Support both Pre-Auth and Post-Auth fraud check workflows
3. Enable transaction lifecycle management (sale, checkout, fulfillment, returns)
4. Maintain consistency with existing Prism interface patterns
5. Support webhook-based asynchronous fraud decisions

## 2. Architecture Design

### 2.1 Service Definition

```protobuf
service FraudService {
  // EvaluatePreAuthorization evaluates fraud risk BEFORE payment authorization.
  // Prevents fraudulent transactions from being submitted to payment processors.
  // High-risk merchants who want to decline suspicious orders before processing payment, reducing chargeback fees.
  rpc EvaluatePreAuthorization(FraudServiceEvaluatePreAuthorizationRequest) 
      returns (FraudServiceEvaluatePreAuthorizationResponse);
  
  // EvaluatePostAuthorization updates fraud case with payment authorization results including AVS/CVV verification.
  // Enriching fraud decisions with actual payment gateway responses for model training and chargeback defense.
  rpc EvaluatePostAuthorization(FraudServiceEvaluatePostAuthorizationRequest) 
      returns (FraudServiceEvaluatePostAuthorizationResponse);
  
  // RecordTransactionData records completed transaction for post-hoc fraud evaluation.
  // Merchants using synchronous payment flows who need fraud protection without separate pre-auth checks.
  rpc RecordTransactionData(FraudServiceRecordTransactionDataRequest) 
      returns (FraudServiceRecordTransactionDataResponse);
  
  // RecordFulfillmentData notifies the fraud provider when an order is shipped with tracking and delivery information.
  // Required for chargeback guarantee protection and improving fraud models with delivery confirmation.
  rpc RecordFulfillmentData(FraudServiceRecordFulfillmentDataRequest) 
      returns (FraudServiceRecordFulfillmentDataResponse);
  
  // RecordReturnData captures customer return and refund information for fraud pattern analysis.
  // Identifying return fraud patterns and enabling fee adjustments on Riskified's chargeback guarantee.
  rpc RecordReturnData(FraudServiceRecordReturnDataRequest) 
      returns (FraudServiceRecordReturnDataResponse);
  
  // Get retrieves the current fraud decision and case status from the provider for polling or reconciliation.
  // Recovering from webhook failures, manual review workflows, and syncing internal state with provider.
  rpc Get(FraudServiceGetRequest) returns (FraudServiceGetResponse);
}
```

**Note**: Exactly 6 RPC methods (no Cancel). Cancel is not uniformly supported by providers and is handled via webhook updates.

### 2.2 Data Flow

Three primary integration patterns:

**Pattern A: Pre-Authorization Fraud Check (EvaluatePreAuthorization → EvaluatePostAuthorization)**
```
Customer Checkout → Pre-Auth API → Fraud Decision → Payment Auth → Post-Auth API → Fulfillment
                          ↓              ↓                              ↓
                    [ACCEPT]      APPROVED                       [SUCCESS]
                    [REJECT]      DECLINED                       [FAILURE]
                    [REVIEW]      Manual Review

Use EvaluatePreAuthorization when: Evaluating fraud BEFORE charging customer's card
Use EvaluatePostAuthorization when: Updating fraud case with payment auth result (AVS/CVV)
```

**Pattern B: Post-Authorization Fraud Check (RecordTransactionData)**
```
Payment Auth → RecordTransactionData API → Fulfillment
                      ↓
                [Fraud Decision]
                (with full payment context)

Use RecordTransactionData when: Payment already captured, evaluate fraud post-hoc
```

**Pattern C: Status Synchronization (Get)**
```
Get API (poll) ←→ Webhook (async) ←→ Get API (reconcile)

Use Get when: Webhook failed, manual review, or status reconciliation
```

### Flow Relationships

| From Flow | To Flow | When to Transition |
|-----------|---------|-------------------|
| EvaluatePreAuthorization | EvaluatePostAuthorization | After payment authorization attempt |
| EvaluatePostAuthorization | RecordFulfillmentData | After successful auth and fraud evaluation |
| RecordTransactionData | RecordFulfillmentData | After fraud evaluation of completed payment |
| RecordFulfillmentData | RecordReturnData | If customer returns shipped items |
| Any | Get | For status polling or reconciliation |

### 2.3 Request/Response ID Pattern

Following the existing pattern from payments and payouts:

| Service | Merchant ID Pattern | Connector ID Pattern |
|---------|---------------------|---------------------|
| Payment | `merchant_transaction_id` | `connector_transaction_id` |
| Payout | `merchant_payout_id` | `connector_payout_id` |
| **Fraud** | `merchant_fraud_id` | `connector_fraud_id` |

## 3. Protocol Buffer Schema

### 3.1 File: `fraud.proto`

```protobuf
syntax = "proto3";

package types;

import "payment.proto";
import "payment_methods.proto";
import "google/protobuf/empty.proto";

option go_package = "github.com/juspay/connector-service/crates/types-traits/grpc-api-types/proto;proto";

// ============================================================================
// FRAUD ENUMERATIONS (Package level - following payment.proto pattern)
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

enum FulfillmentStatus {
  FULFILLMENT_STATUS_UNSPECIFIED = 0;
  FULFILLMENT_STATUS_PENDING = 1;
  FULFILLMENT_STATUS_PARTIAL = 2;
  FULFILLMENT_STATUS_COMPLETE = 3;
  FULFILLMENT_STATUS_REPLACEMENT = 4;
  FULFILLMENT_STATUS_CANCELLED = 5;
}

enum RefundMethod {
  REFUND_METHOD_UNSPECIFIED = 0;
  REFUND_METHOD_STORE_CREDIT = 1;
  REFUND_METHOD_ORIGINAL_PAYMENT_INSTRUMENT = 2;
  REFUND_METHOD_NEW_PAYMENT_INSTRUMENT = 3;
}

// ============================================================================
// CORE FRAUD DATA MESSAGES
// ============================================================================

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

message FraudDestination {
  SecretString full_name = 1;
  optional string organization = 2;
  optional SecretString email = 3;
  Address address = 4;
}

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

message FraudScore {
  int32 score = 1;
  optional string risk_level = 2;
  optional int32 threshold = 3;
}

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
  string device_fingerprint = 16;            // NEW
  string session_id = 17;                    // NEW
  bool synchronous = 18;                     // NEW
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
  string session_id = 4;                     // NEW - was optional
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

### 3.2 Service Registration (in `services.proto`)

```protobuf
import "fraud.proto";

service FraudService {
  rpc EvaluatePreAuthorization(FraudServiceEvaluatePreAuthorizationRequest) 
      returns (FraudServiceEvaluatePreAuthorizationResponse);
  rpc EvaluatePostAuthorization(FraudServiceEvaluatePostAuthorizationRequest) 
      returns (FraudServiceEvaluatePostAuthorizationResponse);
  rpc RecordTransactionData(FraudServiceRecordTransactionDataRequest) 
      returns (FraudServiceRecordTransactionDataResponse);
  rpc RecordFulfillmentData(FraudServiceRecordFulfillmentDataRequest) 
      returns (FraudServiceRecordFulfillmentDataResponse);
  rpc RecordReturnData(FraudServiceRecordReturnDataRequest) 
      returns (FraudServiceRecordReturnDataResponse);
  rpc Get(FraudServiceGetRequest) returns (FraudServiceGetResponse);
}
```

**Note**: Exactly 6 RPC methods. No Cancel method (not uniformly supported by providers).

## 4. Rust Domain Types (Following Payouts Pattern)

### 4.1 Folder Structure

Following the payouts pattern, create:
```
crates/types-traits/domain_types/src/
├── fraud/
│   ├── mod.rs                    (re-exports)
│   ├── fraud_types.rs            (FraudFlowData, request/response types)
│   ├── fraud_method_data.rs      (if needed for provider-specific data)
│   ├── router_request_types.rs   (gRPC request transformations)
│   └── types.rs                  (ForeignTryFrom implementations)
├── connector_flow.rs             (Fraud* flow markers - ADD HERE)
└── lib.rs                        (add 'pub mod fraud;')
```

### 4.2 File: `crates/types-traits/domain_types/src/connector_flow.rs` (Additions)

Add fraud flow markers alongside existing payout flows:

```rust
// ============================================================================
// FRAUD FLOW MARKERS (Add after Payout flows, before FlowName enum)
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

// ============================================================================
// FLOW NAME ENUM (Add fraud variants)
// ============================================================================

#[derive(strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum FlowName {
    // ... existing variants
    Authorize,
    Refund,
    // ... other existing variants
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

### 4.3 File: `crates/types-traits/domain_types/src/fraud/mod.rs`

```rust
pub mod fraud_types;
pub mod router_request_types;
pub mod types;
```

### 4.4 File: `crates/types-traits/domain_types/src/fraud/fraud_types.rs`

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

### 4.5 File: `crates/types-traits/domain_types/src/fraud/router_request_types.rs`

Similar to payouts, define any specific request type conversions needed.

### 4.6 File: `crates/types-traits/domain_types/src/fraud/types.rs`

Similar to payouts, implement `ForeignTryFrom` for gRPC → domain type conversions.

### 4.7 Update: `crates/types-traits/domain_types/src/lib.rs`

```rust
pub mod fraud;  // Add after 'pub mod payouts;'
```

## 5. Interface Traits (connector-integration crate)

**Note**: Following the PaymentService pattern, there is NO separate trait file in the `interfaces` crate. Instead, connector traits are defined in the `connector-integration` crate.

When implementing fraud connectors, follow this pattern in `crates/integrations/connector-integration/src/connectors/{connector}.rs`:

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
> for Signifyd {
    // Implementation
}
```

## 6. Webhook Events

### 6.1 Extend payment.proto WebhookEventType

```protobuf
enum WebhookEventType {
  // ... existing events
  FRM_APPROVED = 28;         // Maps to LEGIT status
  FRM_REJECTED = 29;         // Maps to FRAUD status
  FRM_REVIEW_REQUIRED = 60;  // Maps to MANUAL_REVIEW status
}
```

**Note**: Status mapping:
- `FRM_APPROVED` → `FraudCheckStatus.LEGIT`
- `FRM_REJECTED` → `FraudCheckStatus.FRAUD`
- `FRM_REVIEW_REQUIRED` → `FraudCheckStatus.MANUAL_REVIEW`

## 7. Alignment Summary

| Aspect | Payments | Payouts | Fraud |
|--------|----------|---------|-------|
| Service Name | PaymentService | PayoutService | FraudService |
| Core Flows | 8 flows | 8 flows | 6 flows |
| Status Enum | PaymentStatus | PayoutStatus | FraudCheckStatus |
| Merchant ID | merchant_transaction_id | merchant_payout_id | merchant_fraud_id |
| Connector ID | connector_transaction_id | connector_payout_id | connector_fraud_id |
| Flow Data | PaymentFlowData | PayoutFlowData | FraudFlowData |
| Folder Structure | flat | `payouts/` | `fraud/` (following payouts) |
| Traits in interfaces | No | No | No (following pattern) |

## 8. Implementation Checklist

See `02-implementation-plan.md` for detailed step-by-step implementation instructions.

---

**Next Steps**: 
1. Review implementation plan
2. Begin Phase 1 (Protocol Buffers)
3. Continue with Phase 2 (Domain Types - following payouts pattern)
4. Implement connectors in Phase 3

## 9. Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-04-06 | Initial specification |
| 2.0.0 | 2026-04-06 | Hyperswitch-aligned enums, renamed methods |
| 2.1.0 | 2026-04-06 | **Fixed folder structure** to follow payouts pattern, removed separate interfaces traits |
