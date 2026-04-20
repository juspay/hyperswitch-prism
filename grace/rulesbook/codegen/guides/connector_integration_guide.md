# UCS Connector Integration: Comprehensive Step-by-Step Guide

This guide provides a complete, resumable process for integrating payment connectors into the UCS (Universal Connector Service) system. It supports all payment methods and flows, and can be used to continue partial implementations.

> **Important:** This guide is UCS-specific. The architecture differs significantly from traditional Hyperswitch implementations.

## 🏗️ UCS Architecture Overview

### Key Components
```rust
// Core UCS imports for all connectors
use domain_types::{
    connector_flow::{Authorize, Capture, Void, Refund, PSync, RSync},
    connector_types::{
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentVoidData,
        RefundsData, PaymentsSyncData, RefundSyncData,
        PaymentsResponseData, RefundsResponseData, RequestDetails, ResponseId
    },
    router_data_v2::RouterDataV2,
    router_response_types::Response,
};
use interfaces::connector_integration_v2::ConnectorIntegrationV2;
```

### UCS-Specific Patterns
- **RouterDataV2**: Enhanced type-safe data handling
- **ConnectorIntegrationV2**: Modern trait-based integration
- **Domain Types**: Centralized domain modeling
- **gRPC-first**: All communication via Protocol Buffers
- **Stateless**: No database dependencies

### 🛠️ Utility Functions
UCS provides comprehensive utility functions to avoid code duplication and maintain consistency:
- **Error Handling**: `missing_field_err`, `handle_json_response_deserialization_failure`
- **Amount Conversion**: `convert_amount`, `to_currency_base_unit`, amount convertors
- **Data Transformation**: `to_connector_meta_from_secret`, `convert_uppercase`
- **XML/JSON Processing**: `preprocess_xml_response_bytes`, `serialize_to_xml_string_with_root`
- **Card Processing**: `get_card_details`, `get_card_issuer`
- **Date/Time**: `now`, `get_timestamp_in_milliseconds`, `format_date`

> **📖 Complete Reference:** See [`guides/utility_functions_reference.md`](utility_functions_reference.md) for comprehensive mapping of all utility functions with examples and use cases.

## 🎯 Connector Implementation States

### State Assessment
Before starting, determine your current implementation state:

1. **Fresh Start**: No implementation exists
2. **Partial Core**: Basic auth and authorize flow implemented
3. **Core Complete**: All basic flows working (auth, capture, void, refund)
4. **Extended**: Advanced flows and multiple payment methods
5. **Near Complete**: Only specific flows or payment methods missing
6. **Debug/Fix**: Implementation exists but has issues

## 📋 Complete Flow Coverage

### Core Payment Flows (Priority 1)
- **Authorize**: Initial payment authorization
- **Capture**: Capture authorized amounts
- **Void**: Cancel authorized payments
- **Refund**: Process refunds (full/partial)
- **PSync**: Payment status synchronization
- **RSync**: Refund status synchronization

### Advanced Flows (Priority 2)
- **CreateOrder**: Multi-step payment initiation
- **ServerSessionAuthenticationToken**: Secure session management
- **SetupMandate**: Recurring payment setup
- **RepeatPayment**: Process recurring payments using stored mandates
- **DefendDispute**: Handle chargeback disputes
- **SubmitEvidence**: Submit dispute evidence

### Webhook Integration (Priority 3)
- **IncomingWebhook**: Real-time payment notifications
- **WebhookSourceVerification**: Signature validation
- **EventMapping**: Webhook event to status mapping

## 💳 Payment Method Support

### Card Payments
```rust
PaymentMethodData::Card(card_data) => {
    // Handle all card networks: Visa, Mastercard, Amex, Discover, etc.
    // Handle CVV verification
}
```

## 🛠️ Implementation Process

### Phase 1: Preparation and Planning

#### Step 1.1: Analyze Current State
If resuming partial implementation:
```bash
# AI Command: "analyze current state of [ConnectorName] in UCS"
# The AI will examine existing code and identify:
# - Implemented flows
# - Supported payment methods  
# - Missing functionality
# - Code quality issues
```

#### Step 1.2: Create/Update Technical Specification
```bash
# For new implementation:
# Use: grace-ucs/connector_integration/template/tech_spec.md //change

# For continuing implementation:
# AI will update existing spec with missing components
```

#### Step 1.3: Implementation Planning
```bash
# AI will create detailed plan based on:
# - Current implementation state
# - Missing functionality
# - Priority of remaining work
# Use: grace-ucs/connector_integration/template/planner_steps.md
```

### Phase 2: Core Implementation

#### Step 2.1: Connector Structure Setup
```rust
// File: crates/integrations/connector-integration/src/connectors/connector_name.rs

#[derive(Debug, Clone)]
pub struct ConnectorName;

impl ConnectorCommon for ConnectorName {
    fn id(&self) -> &'static str {
        "connector_name"
    }
    
    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.connector_name.base_url.as_ref()
    }
    
    fn get_currency_unit(&self) -> api::CurrencyUnit {
        api::CurrencyUnit::Minor // or Base, depending on connector
    }
    
    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }
    
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        // UCS-specific error handling
    }
}
```

#### Step 2.2: Authentication Implementation
```rust
#[derive(Debug, Clone)]
pub struct ConnectorNameAuthType {
    pub api_key: SecretSerdeValue,
    // Add other auth fields as needed
}

impl TryFrom<&ConnectorAuthType> for ConnectorNameAuthType {
    type Error = Error;
    
    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        // Implementation for auth type conversion
    }
}
```

### Phase 3: Flow Implementation

> **📖 Pattern Reference:** For detailed implementation patterns, see:
> - **Authorization Flow**: `guides/patterns/pattern_authorize.md`
> - **Capture Flow**: `guides/patterns/pattern_capture.md`
> - **Refund Flow**: `guides/patterns/pattern_refund.md`
> - **Void Flow**: `guides/patterns/pattern_void.md`
> - **Psync Flow**: `guides/patterns/pattern_psync.md`
> - **Rsync Flow**: `guides/patterns/pattern_rsync.md`
> - **SetupMandate Flow**: `guides/patterns/pattern_setup_mandate.md`
> - **RepeatPayment Flow**: `guides/patterns/repeat_payment_flow_patterns.md`

## 🔄 Resuming Partial Implementation

### Common Resume Scenarios

#### "I have authorize working, need to add capture"
```bash
# AI Command: "add capture flow to existing [ConnectorName] connector in UCS"
# AI will:
# 1. Analyze existing authorize implementation
# 2. Use patterns from guides/patterns/pattern_capture.md
# 3. Create capture flow following same patterns
# 4. Ensure consistency with existing code style
```

## 🚨 Common UCS Pitfalls

### 1. RouterData vs RouterDataV2
```rust
// WRONG (traditional Hyperswitch)
RouterData<Flow, Request, Response>

// CORRECT (UCS)
RouterDataV2<Flow, Request, Response>
```

### 2. Trait Implementation
```rust
// WRONG (traditional)
ConnectorIntegration<Flow, Request, Response>

// CORRECT (UCS)
ConnectorIntegrationV2<Flow, Request, Response>
```

### 3. Error Handling
```rust
// UCS uses domain_types errors, not hyperswitch_domain_models
use domain_types::errors;
```

### 4. Import Paths
```rust
// UCS-specific imports
use domain_types::*;
use interfaces::connector_integration_v2::*;
// NOT hyperswitch_interfaces or hyperswitch_domain_models
```

## 📊 Implementation Checklist

### Core Implementation ✅
- [ ] Connector structure and auth
- [ ] Authorize flow
- [ ] Capture flow  
- [ ] Void flow
- [ ] Refund flow
- [ ] Payment sync
- [ ] Refund sync
- [ ] Error handling

### Payment Methods ✅
- [ ] Card payments (all networks)

### Quality & Testing ✅
- [ ] cargo build works for all flows

## 🎯 Success Metrics

A complete UCS connector implementation should:
1. **Support all relevant payment methods** for the connector
2. **Handle all core flows** (auth, capture, void, refund, sync)
3. **Follow UCS patterns** consistently
4. **Handle errors gracefully** with proper mapping
5. **Should be error free** should build successfully without any compilation errors

Remember: GRACE makes connector development resumable at any stage. You can always continue where you left off!