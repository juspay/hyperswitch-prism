# Proto Interface Validation Analysis

## Document Information
- **Version**: 4.1.0
- **Date**: 2026-04-06
- **Status**: Final - Hyperswitch-Aligned
- **Scope**: Validation of fraud.proto against Signifyd and Riskified APIs with Hyperswitch-aligned enums

## Executive Summary

This document validates the fraud interface proto specification against Signifyd and Riskified APIs with strict adherence to Hyperswitch's existing enums.

### Critical Constraints
1. **No New States**: All enums MUST match Hyperswitch exactly - no additions allowed
2. **Method Naming**: Updated to verb-noun format for clarity
3. **Provider Mapping**: Each proto method maps to specific provider endpoints
4. **Architecture**: Follows PaymentService/Payouts pattern - no new patterns

### Current Specification

| Proto Element | Hyperswitch Match | Provider Support | Location Pattern |
|--------------|-------------------|------------------|------------------|
| `FraudCheckStatus` | ✅ Exact match (5 states) | Signifyd, Riskified | Proto + `fraud/fraud_types.rs` |
| `FraudAction` | ✅ Exact match (2 actions) | Signifyd, Riskified | Proto + `fraud/fraud_types.rs` |
| `FraudEvaluatePreAuthorization` | New method | Signifyd /checkouts, Riskified /submit | `connector_flow.rs` marker |
| `FraudEvaluatePostAuthorization` | New method | Signifyd /transactions, Riskified /update | `connector_flow.rs` marker |
| `FraudRecordTransactionData` | New method | Signifyd /sales, Riskified /create | `connector_flow.rs` marker |
| `FraudRecordFulfillmentData` | New method | Signifyd /fulfillments, Riskified /fulfill | `connector_flow.rs` marker |
| `FraudRecordReturnData` | New method | Signifyd /returns, Riskified /partial_refund | `connector_flow.rs` marker |
| `FraudGet` | Existing pattern | Signifyd /decisions/{id} | `connector_flow.rs` marker |

---

## 1. Status Enum Alignment

### 1.1 FraudCheckStatus (Hyperswitch-Defined)

```rust
// From Hyperswitch: crates/common_enums/src/enums.rs
pub enum FraudCheckStatus {
    Fraud,              // Confirmed fraudulent
    ManualReview,       // Under manual review
    #[default]
    Pending,            // Awaiting decision
    Legit,              // Confirmed legitimate
    TransactionFailure, // Payment/auth failed
}
```

**Proto Definition**:
```protobuf
enum FraudCheckStatus {
  FRAUD_CHECK_STATUS_UNSPECIFIED = 0;
  FRAUD_CHECK_STATUS_PENDING = 1;
  FRAUD_CHECK_STATUS_FRAUD = 2;
  FRAUD_CHECK_STATUS_LEGIT = 3;
  FRAUD_CHECK_STATUS_MANUAL_REVIEW = 4;
  FRAUD_CHECK_STATUS_TRANSACTION_FAILURE = 5;
}
```

**Rust Domain Type** (in `fraud/fraud_types.rs`):
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FraudCheckStatus {
    Pending,
    Fraud,
    Legit,
    ManualReview,
    TransactionFailure,
}
```

**Validation**:
- ✅ No new states introduced
- ✅ Exactly matches Hyperswitch definition
- ✅ Proto and Rust domain type aligned
- ✅ Provider states mappable to these 5 states

**Provider Mapping**:

| Hyperswitch | Signifyd | Riskified |
|-------------|----------|-----------|
| `PENDING` | `PENDING` | `pending` / `reviewing` |
| `FRAUD` | `REJECT` (fraud signals) | `declined` / `canceled` |
| `LEGIT` | `ACCEPT` | `approved` |
| `MANUAL_REVIEW` | `REVIEW` | `review` / `pending` |
| `TRANSACTION_FAILURE` | Payment failure | Gateway error |

---

### 1.2 FraudAction (Hyperswitch-Defined)

```rust
// From Hyperswitch: crates/common_enums/src/enums.rs
pub enum FraudAction {
    Accept,     // Approve transaction
    Reject,     // Decline transaction
}
```

**Proto Definition**:
```protobuf
enum FraudAction {
  FRAUD_ACTION_UNSPECIFIED = 0;
  FRAUD_ACTION_ACCEPT = 1;
  FRAUD_ACTION_REJECT = 2;
}
```

**Rust Domain Type** (in `fraud/fraud_types.rs`):
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FraudAction {
    Accept,
    Reject,
}
```

**Validation**:
- ✅ Exactly 2 actionable values (ACCEPT, REJECT) plus UNSPECIFIED placeholder in proto
- ✅ Rust domain type matches Hyperswitch exactly
- ✅ All providers support accept/reject semantics
- ✅ Riskified's "review" state maps to REJECT action with MANUAL_REVIEW status

**Provider Action Mapping**:

| Hyperswitch | Signifyd | Riskified |
|-------------|----------|-----------|
| `ACCEPT` | `ACCEPT` | `approved` |
| `REJECT` | `REJECT` | `declined` / `canceled` |

---

## 2. Method-to-Provider API Mapping

### 2.1 Signifyd API Mapping

| Proto Method | Signifyd Endpoint | HTTP | Purpose |
|--------------|-------------------|------|---------|
| `EvaluatePreAuthorization` | `/v3/checkouts` | POST | Pre-auth fraud evaluation |
| `EvaluatePostAuthorization` | `/v3/transactions` | POST | Post-auth case update |
| `RecordTransactionData` | `/v3/sales` | POST | Combined transaction recording |
| `RecordFulfillmentData` | `/v3/fulfillments` | POST | Shipment notification |
| `RecordReturnData` | `/v3/returns` | POST | Return/refund recording |
| `Get` | `/v3/decisions/{orderId}` | GET | Decision retrieval |

**Status Translation**:
```
Signifyd Decision → Hyperswitch Status
---------------------------------------
ACCEPT           → LEGIT
REJECT           → FRAUD
REVIEW           → MANUAL_REVIEW
ERROR/FAILED     → TRANSACTION_FAILURE
```

---

### 2.2 Riskified API Mapping

| Proto Method | Riskified Endpoint | HTTP | Mode |
|--------------|-------------------|------|------|
| `EvaluatePreAuthorization` | `/api/orders/submit` | POST | Async (webhook) |
| `EvaluatePreAuthorization` | `/api/orders/decide` | POST | Sync (immediate) |
| `EvaluatePostAuthorization` | `/api/orders/update` | POST | Post-auth update |
| `RecordTransactionData` | `/api/orders/create` | POST | Transaction record |
| `RecordFulfillmentData` | `/api/orders/fulfill` | POST | Fulfillment |
| `RecordReturnData` | `/api/orders/partial_refund` | POST | Returns |
| `Get` | N/A (webhook only) | - | Decision via webhook |

**Status Translation**:
```
Riskified Decision → Hyperswitch Status
----------------------------------------
approved           → LEGIT
declined           → FRAUD
canceled           → FRAUD
pending/review     → MANUAL_REVIEW
gateway_error      → TRANSACTION_FAILURE
```

---

## 3. Architecture Pattern Validation

### 3.1 Folder Structure (Following Payouts Pattern)

```
crates/types-traits/domain_types/src/
├── fraud/                      ← NEW (following payouts/ pattern)
│   ├── mod.rs                  (re-exports)
│   ├── fraud_types.rs          (FraudFlowData, enums, types)
│   ├── router_request_types.rs (fraud-specific request types)
│   └── types.rs                (ForeignTryFrom implementations)
├── connector_flow.rs           (Fraud* flow markers added here)
└── lib.rs                      (pub mod fraud;)
```

**Validation**:
- ✅ Follows payouts folder structure exactly
- ✅ Flow markers in `connector_flow.rs` (not in fraud_types.rs)
- ✅ No separate trait file (following PaymentService pattern)

### 3.2 Flow Marker Location

**Correct** (following existing pattern):
```rust
// crates/types-traits/domain_types/src/connector_flow.rs

#[derive(Debug, Clone)]
pub struct FraudEvaluatePreAuthorization;

#[derive(Debug, Clone)]
pub struct FraudEvaluatePostAuthorization;
// ... etc

#[derive(strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum FlowName {
    // ... existing variants
    FraudEvaluatePreAuthorization,
    FraudEvaluatePostAuthorization,
    FraudRecordTransactionData,
    FraudRecordFulfillmentData,
    FraudRecordReturnData,
    FraudGet,
}
```

### 3.3 NO Separate interfaces/fraud.rs

**Following PaymentService Pattern**:
- ❌ NO `interfaces/src/fraud.rs` file
- ❌ NO `FraudConnectorTrait` trait
- ✅ Implement `ConnectorIntegrationV2` directly in connector files

Example connector implementation:
```rust
// crates/integrations/connector-integration/src/connectors/signifyd.rs

impl ConnectorIntegrationV2<
    connector_flow::FraudEvaluatePreAuthorization,
    FraudFlowData,
    FraudEvaluatePreAuthorizationRequest,
    FraudEvaluatePreAuthorizationResponse,
> for Signifyd {
    // Implementation
}
```

---

## 4. Required Field Analysis

### 4.1 Critical Fields Added

**EvaluatePreAuthorizationRequest**:
```protobuf
string device_fingerprint = 16;  // REQUIRED for Signifyd
string session_id = 17;          // REQUIRED for both providers
bool synchronous = 18;           // REQUIRED for Riskified mode selection
```

**Impact**: 
- Signifyd: Cannot perform device-based risk analysis without fingerprint
- Riskified: Cannot distinguish sync/async mode

**All Required Fields (Must be present)**:

| Message | Required Fields | Reason |
|---------|-----------------|--------|
| `EvaluatePreAuthorizationRequest` | `merchant_fraud_id`, `order_id`, `customer`, `browser_info`, `device_fingerprint`, `session_id` | Provider requirements |
| `EvaluatePostAuthorizationRequest` | `merchant_fraud_id`, `order_id`, `connector_transaction_id`, `session_id` | Link to payment |
| `RecordTransactionDataRequest` | `merchant_fraud_id`, `order_id`, `session_id` | Case creation |
| `RecordFulfillmentDataRequest` | `merchant_fraud_id`, `order_id`, `session_id` | Case lookup |
| `RecordReturnDataRequest` | `merchant_fraud_id`, `order_id`, `session_id` | Return record |
| `GetRequest` | `merchant_fraud_id`, `order_id` | Lookup requirement |

---

## 5. Status Compatibility Matrix

### 5.1 Can All Provider States Map to Hyperswitch?

| Provider State | Hyperswitch Mapping | Supported |
|----------------|---------------------|-----------|
| **Signifyd ACCEPT** | LEGIT | ✅ |
| **Signifyd REJECT** | FRAUD | ✅ |
| **Signifyd REVIEW** | MANUAL_REVIEW | ✅ |
| **Signifyd PENDING** | PENDING | ✅ |
| **Signifyd ERROR** | TRANSACTION_FAILURE | ✅ |
| **Riskified approved** | LEGIT | ✅ |
| **Riskified declined** | FRAUD | ✅ |
| **Riskified canceled** | FRAUD | ✅ |
| **Riskified pending** | PENDING/MANUAL_REVIEW | ✅ |
| **Riskified review** | MANUAL_REVIEW | ✅ |

**Conclusion**: ✅ All provider states mappable to Hyperswitch's 5 states

---

## 6. Method Responsibilities

### 6.1 EvaluatePreAuthorization
- **Purpose**: Evaluate fraud BEFORE payment authorization
- **Providers**: Signifyd (/checkouts), Riskified (/submit or /decide)
- **Returns**: Decision mapped to FraudCheckStatus
- **Key Fields**: device_fingerprint, session_id, customer, amount
- **Synchronous Flag**: Riskified uses this to choose between /submit (async) and /decide (sync)

### 6.2 EvaluatePostAuthorization
- **Purpose**: Update fraud case with payment auth results
- **Providers**: Signifyd (/transactions), Riskified (/update)
- **Input**: Authorization result (success/failure), AVS/CVV data
- **Updates**: Existing case with payment gateway response

### 6.3 RecordTransactionData
- **Purpose**: Record completed transaction for post-hoc evaluation
- **Providers**: Signifyd (/sales), Riskified (/create)
- **Use Case**: Synchronous flows where fraud check happens after payment
- **Combines**: Purchase data + transaction result in single call

### 6.4 RecordFulfillmentData
- **Purpose**: Notify fraud provider of shipment
- **Providers**: Signifyd (/fulfillments), Riskified (/fulfill)
- **Required For**: Chargeback guarantee protection
- **Includes**: Tracking numbers, carrier, shipping address

### 6.5 RecordReturnData
- **Purpose**: Record customer returns
- **Providers**: Signifyd (/returns), Riskified (/partial_refund)
- **Use Case**: Return fraud detection, fee adjustments

### 6.6 Get
- **Purpose**: Retrieve fraud decision
- **Providers**: Signifyd (/decisions/{id}), Riskified (webhook fallback)
- **Use Case**: Webhook recovery, status sync, manual review

---

## 7. Implementation Recommendations

### 7.1 Priority 1 (Critical)

1. **Add device_fingerprint field** (DONE in proto)
   ```protobuf
   message FraudServiceEvaluatePreAuthorizationRequest {
     // ... existing fields
     string device_fingerprint = 16;
   }
   ```

2. **Add session_id to all requests** (DONE in proto)
   ```protobuf
   // Added to: EvaluatePreAuthorization, EvaluatePostAuthorization,
   // RecordTransactionData, RecordFulfillmentData, RecordReturnData
   string session_id = X;
   ```

3. **Add synchronous flag for Riskified** (DONE in proto)
   ```protobuf
   message FraudServiceEvaluatePreAuthorizationRequest {
     // ... existing fields
     bool synchronous = 18;  // true=decide, false=submit
   }
   ```

### 7.2 Priority 2 (Architecture)

4. **Follow Payouts Folder Structure** ✅
   - Create `fraud/` subdirectory in `domain_types/src/`
   - Place `fraud_types.rs`, `types.rs`, `router_request_types.rs` there

5. **Flow Markers in connector_flow.rs** ✅
   - Add `FraudEvaluatePreAuthorization`, etc. to `connector_flow.rs`
   - Add variants to `FlowName` enum

6. **NO interfaces/fraud.rs** ✅
   - Following PaymentService pattern
   - Implement traits directly in connector files

### 7.3 Priority 3 (Nice to Have)

7. **Enhanced FraudScore** (if needed - does NOT add status)
   ```protobuf
   message FraudScore {
     int32 score = 1;
     optional string provider_scale = 2;
   }
   ```

---

## 8. Service Definition

```protobuf
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

---

## 9. Validation Summary

### ✅ Aligned with Hyperswitch
- [x] FraudCheckStatus: 5 states exactly match
- [x] FraudAction: 2 actions exactly match
- [x] No new states introduced
- [x] All provider states mappable

### ✅ Architecture Pattern Correct
- [x] Follows payouts folder structure (`fraud/` subdirectory)
- [x] Flow markers in `connector_flow.rs`
- [x] NO separate trait file in `interfaces` (PaymentService pattern)
- [x] Domain types in `fraud/fraud_types.rs`

### ✅ Required Fields Present
- [x] device_fingerprint field added
- [x] session_id added to all requests
- [x] synchronous flag for Riskified

### ❌ Removed (Intentionally)
- Cancel method (providers don't support uniformly)
- Extra status states (CHALLENGE, CANCELLED, ERROR, TIMEOUT, APPROVED, REJECTED)
- FraudCheckType enum (redundant with method names)
- Separate interfaces/fraud.rs (following PaymentService pattern)

---

## 10. Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-04-06 | Initial validation analysis |
| 2.0.0 | 2026-04-06 | Removed CyberSource DM |
| 3.0.0 | 2026-04-06 | Updated for renamed methods |
| 4.0.0 | 2026-04-06 | Hyperswitch-aligned enums, removed extra states |
| 4.1.0 | 2026-04-06 | **Added architecture pattern validation**, confirmed NO interfaces/fraud.rs |
| 4.2.0 | 2026-04-07 | Fixed flow marker derives (`Clone` not `Copy`) to match existing patterns |
