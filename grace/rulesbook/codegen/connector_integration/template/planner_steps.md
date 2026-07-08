You are an expert UCS (Unified Connector Service) implementation planner tasked with creating detailed, step-by-step implementation plans for payment connectors.

Your plan will be used to guide AI systems through the complete implementation process, including resuming partial implementations and adding missing features.

First, review the technical specification:

<technical_specification>
{{tech_spec_content}}
</technical_specification>

<current_implementation_state>
{{current_state}} 
<!-- This will be filled with:
- "fresh_start" for new implementations
- Detailed analysis of existing implementation for partial continuations
- Specific missing features for feature additions
- Error descriptions for debugging scenarios
-->
</current_implementation_state>

<project_rules>
1. **UCS Architecture**: All plans must use UCS-specific patterns (RouterDataV2, ConnectorIntegrationV2, domain-types)
2. **gRPC-First**: Focus on gRPC integration, not REST APIs
3. **Resumable Planning**: Plans must accommodate continuing from any implementation state
4. **Complete Coverage**: Address ALL payment methods and flows the connector supports
5. **Modular Implementation**: Break down into manageable, independent modules
6. **Testing Integration**: Include gRPC testing at each step
7. **Error Handling**: Comprehensive error mapping and handling
8. **State Tracking**: Clear markers for implementation progress
9. **Payment Method Priority**: Implement based on connector's primary use cases
10. **Documentation**: Include implementation notes for future continuation
</project_rules>

<ucs_implementation_phases>
**Phase 1: Foundation Setup**
- Connector structure and authentication
- Basic trait implementations
- Error handling framework

**Phase 2: Core Payment Flows**
- Authorize flow implementation
- Capture flow implementation  
- Void flow implementation
- Basic payment method support (usually cards)

**Phase 3: Extended Payment Flows**
- Refund flow implementation
- Payment sync (PSync) implementation
- Refund sync (RSync) implementation
- Status mapping completion

**Phase 4: Payment Method Expansion**
- Digital wallet support (Apple Pay, Google Pay, etc.)
- Bank transfer methods
- BNPL provider integrations
- Regional payment methods

**Phase 5: Advanced Features**
- Webhook implementation
- 3DS authentication handling
- Recurring payment setup (SetupMandate flow)
- Recurring payment processing (RepeatPayment flow)
- Multi-step payment flows

**Phase 6: Production Readiness**
- Comprehensive testing
- Performance optimization
- Error scenario handling
- Documentation completion
</ucs_implementation_phases>

<output_file>
Store the implementation plan in grace-ucs/connector_integration/{{connector_name}}/{{connector_name}}_plan.md
</output_file>

Your task is to create a detailed, step-by-step implementation plan. Consider the current implementation state and create a plan that:

1. **Assesses Current State** (if continuing partial implementation)
2. **Prioritizes Remaining Work** based on importance and dependencies
3. **Provides Specific Implementation Steps** with code examples
4. **Includes Testing Strategy** for each phase
5. **Handles Error Scenarios** throughout the process
6. **Enables Easy Resumption** with clear progress markers

## UCS Implementation Plan Template

Generate the implementation plan using this structure:

```markdown
# {{connector_name}} UCS Connector Implementation Plan

## 📊 Implementation State Assessment

### Current State: {{current_state}}

**✅ Completed Components:**
- [List what's already implemented]
- [Include flow coverage, payment methods, etc.]

**🔄 In Progress Components:**
- [List partially implemented features]
- [Note what needs completion]

**❌ Missing Components:**
- [List what needs to be implemented]
- [Prioritize by importance]

**🐛 Known Issues:**
- [List any bugs or problems]
- [Include error scenarios]

## 📝 Progress Tracking

**IMPORTANT: AI must update this section after completing each step with the format:**
`- [✅ COMPLETED] Step Name: Implemented [specific_details] in [file_location]`

**Implementation Progress:**
- [ ] Template generation (if needed)
- [ ] Core flow implementations
- [ ] Payment method support
- [ ] Error handling
- [ ] Testing
- [ ] Documentation

## 🎯 Implementation Strategy

### Priority Order:
1. **Critical Path**: [Most important features first]
2. **Dependencies**: [Features that unlock other features]
3. **Payment Method Priority**: [Based on connector's main use cases]
4. **Advanced Features**: [Nice-to-have features]

### Risk Assessment:
- **High Risk**: [Complex integrations, 3DS, webhooks]
- **Medium Risk**: [Multiple payment methods, error handling]
- **Low Risk**: [Basic flows, standard implementations]

## 📋 Detailed Implementation Steps

### Phase 1: Foundation Setup
*Target: Establish basic connector structure*

#### Step 1.1: Project Structure Setup
**Objective**: Create UCS connector file structure

**Implementation:**
```rust
// File: crates/integrations/connector-integration/src/connectors/{{connector_name}}.rs
// - Basic connector struct
// - ConnectorCommon trait implementation
// - Authentication type definition
```

**Tasks:**
- [ ] Create connector main file
- [ ] Implement ConnectorCommon trait
- [ ] Define authentication struct
- [ ] Add to connectors module
- [ ] Set up transformers module

**Testing:**
- [ ] Verify basic compilation
- [ ] Test authentication parsing

**Progress Updates:**
[AI will add completion status here with format: [✅ COMPLETED] Task: Details]

**User Feedback:**
[AI will request optional feedback after flow implementation and store here]

**Code Example:**
```rust
#[derive(Debug, Clone)]
pub struct {{ConnectorName}};

impl ConnectorCommon for {{ConnectorName}} {
    fn id(&self) -> &'static str {
        "{{connector_name}}"
    }
    
    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.{{connector_name}}.base_url.as_ref()
    }
    
    fn get_currency_unit(&self) -> api::CurrencyUnit {
        // Based on tech spec analysis
        api::CurrencyUnit::Minor
    }
    
    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }
}
```

#### Step 1.2: Authentication Implementation
**Objective**: Implement connector-specific authentication

**Implementation:**
```rust
// Define auth type based on connector requirements
#[derive(Debug, Clone)]
pub struct {{ConnectorName}}AuthType {
    pub api_key: SecretSerdeValue,
    // Add other auth fields based on tech spec
}

impl TryFrom<&ConnectorAuthType> for {{ConnectorName}}AuthType {
    type Error = Error;
    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        // Implementation based on connector auth method
    }
}
```

**Tasks:**
- [ ] Define auth struct based on connector requirements
- [ ] Implement TryFrom conversion
- [ ] Add auth header generation
- [ ] Test auth with connector's test environment

#### Step 1.3: Error Handling Framework
**Objective**: Set up comprehensive error handling

**Implementation:**
```rust
impl ConnectorCommon for {{ConnectorName}} {
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        // Parse connector error response
        // Map to UCS error types
        // Include all required fields
    }
}
```

**Tasks:**
- [ ] Define connector error response struct
- [ ] Map connector errors to UCS error types
- [ ] Implement build_error_response
- [ ] Test error scenarios

### Phase 2: Core Payment Flows
*Target: Implement basic payment operations*

#### Step 2.1: Authorize Flow Implementation
**Objective**: Implement payment authorization

**Implementation:**
```rust
impl ConnectorIntegrationV2<Authorize, PaymentsAuthorizeData, PaymentsResponseData>
    for {{ConnectorName}}
{
    // Full trait implementation
}
```

**Tasks:**
- [ ] Implement get_headers method
- [ ] Implement get_url method
- [ ] Implement get_request_body method
- [ ] Implement build_request method
- [ ] Implement handle_response method
- [ ] Create request/response structs in transformers
- [ ] Handle basic payment methods (start with cards)

**Payment Method Support:**
- [ ] Card payments (Visa, Mastercard, Amex)
- [ ] Basic validation and transformation
- [ ] Amount and currency handling
- [ ] Customer data transformation

**Testing:**
- [ ] Unit tests for request transformation
- [ ] Unit tests for response transformation
- [ ] gRPC integration test for successful authorization
- [ ] gRPC integration test for failed authorization

#### Step 2.2: Capture Flow Implementation
**Objective**: Implement payment capture

**Tasks:**
- [ ] Implement ConnectorIntegrationV2 for Capture
- [ ] Handle partial and full captures
- [ ] Create capture request/response structs
- [ ] Test capture scenarios

#### Step 2.3: Void Flow Implementation
**Objective**: Implement payment cancellation

**Tasks:**
- [ ] Implement ConnectorIntegrationV2 for Void
- [ ] Handle void/cancel operations
- [ ] Create void request/response structs
- [ ] Test void scenarios

### Phase 3: Extended Payment Flows
*Target: Complete all basic payment operations*

#### Step 3.1: Refund Flow Implementation
**Objective**: Implement refund processing

**Tasks:**
- [ ] Implement ConnectorIntegrationV2 for Refund
- [ ] Handle partial and full refunds
- [ ] Create refund request/response structs
- [ ] Map refund statuses to UCS types
- [ ] Test refund scenarios

#### Step 3.2: Sync Operations Implementation
**Objective**: Implement status synchronization

**Tasks:**
- [ ] Implement PSync (payment status sync)
- [ ] Implement RSync (refund status sync)
- [ ] Handle status polling and updates
- [ ] Test sync operations

### Phase 4: Payment Method Expansion
*Target: Support all payment methods the connector offers*

#### Step 4.1: Digital Wallet Support
**Objective**: Implement wallet payments

**Priority Wallets (implement in order):**
- [ ] Apple Pay
- [ ] Google Pay  
- [ ] PayPal
- [ ] [Add other wallets based on connector support]

**Implementation per wallet:**
```rust
PaymentMethodData::Wallet(wallet_data) => match wallet_data {
    WalletData::ApplePay(apple_pay) => {
        // Apple Pay specific transformation
        Self::build_apple_pay_request(item, apple_pay)
    }
    // ... other wallets
}
```

**Tasks per wallet:**
- [ ] Request transformation
- [ ] Response handling
- [ ] Specific validation rules
- [ ] Test cases

#### Step 4.2: Bank Transfer Methods
**Objective**: Implement bank transfer payments

**Priority Methods:**
- [ ] ACH (if US market)
- [ ] SEPA (if European market)
- [ ] Local bank transfers (based on connector's markets)

#### Step 4.3: BNPL Provider Integration
**Objective**: Implement Buy Now Pay Later options

**Priority Providers:**
- [ ] Klarna (if supported)
- [ ] Affirm (if supported)
- [ ] Afterpay (if supported)
- [ ] [Regional BNPL providers based on markets]

#### Step 4.4: Regional Payment Methods
**Objective**: Implement region-specific payment methods

**Based on Connector's Markets:**
- [ ] UPI (India)
- [ ] Alipay (China)
- [ ] WeChat Pay (China)
- [ ] [Other regional methods]

### Phase 5: Advanced Features
*Target: Implement advanced connector capabilities*

#### Step 5.1: Webhook Implementation
**Objective**: Implement real-time webhook handling

**Condition**: Only if connector supports webhooks

**Tasks:**
- [ ] Implement IncomingWebhook trait
- [ ] Implement webhook signature verification
- [ ] Map webhook events to UCS events
- [ ] Handle payment and refund webhooks
- [ ] Test webhook scenarios

**Implementation:**
```rust
impl IncomingWebhook for {{ConnectorName}} {
    fn get_webhook_object_reference_id(
        &self,
        request: &IncomingWebhookRequestDetails<'_>,
    ) -> CustomResult<ObjectReferenceId, errors::IntegrationError> {
        // Extract payment/refund ID from webhook
    }
    
    fn get_webhook_event_type(
        &self,
        request: &IncomingWebhookRequestDetails<'_>,
    ) -> CustomResult<IncomingWebhookEvent, errors::IntegrationError> {
        // Map connector events to UCS events
    }
}
```

#### Step 5.2: 3DS Authentication
**Objective**: Implement 3DS authentication flows

**Condition**: If connector supports 3DS

**Tasks:**
- [ ] Handle 3DS initiation
- [ ] Process authentication challenges
- [ ] Complete authentication flow
- [ ] Test 3DS scenarios

#### Step 5.3: Recurring Payments
**Objective**: Implement mandate setup and recurring payment processing

**Condition**: If connector supports recurring payments

**Tasks:**
- [ ] Implement SetupMandate flow for mandate creation
- [ ] Implement RepeatPayment flow for processing recurring payments
- [ ] Handle mandate reference storage and retrieval
- [ ] Process subsequent payments using stored mandates
- [ ] Test mandate setup scenarios
- [ ] Test repeat payment scenarios

#### Step 5.4: Multi-Step Payment Flows
**Objective**: Implement complex payment flows

**Condition**: If connector requires multi-step flows

**Tasks:**
- [ ] Implement CreateOrder flow
- [ ] Handle order completion
- [ ] Manage payment state transitions
- [ ] Test multi-step scenarios

### Phase 6: Production Readiness
*Target: Ensure production-quality implementation*

#### Step 6.1: Comprehensive Testing
**Objective**: Achieve full test coverage

**Test Categories:**
- [ ] Unit tests for all transformers
- [ ] gRPC integration tests for all flows
- [ ] Error scenario testing
- [ ] Payment method specific tests
- [ ] Webhook testing (if applicable)
- [ ] Performance testing

**Test Structure:**
```rust
// File: crates/grpc-server/grpc-server/tests/{{connector_name}}_test.rs

#[tokio::test]
async fn test_payment_authorize_card_success() {
    // Test card authorization via gRPC
}

#[tokio::test]
async fn test_payment_authorize_apple_pay() {
    // Test Apple Pay authorization
}

// ... comprehensive test suite
```

#### Step 6.2: Error Scenario Handling
**Objective**: Handle all possible error scenarios

**Error Categories:**
- [ ] Network errors
- [ ] Authentication errors
- [ ] Validation errors
- [ ] Business logic errors
- [ ] Timeout scenarios

#### Step 6.3: Performance Optimization
**Objective**: Optimize for production performance

**Tasks:**
- [ ] Request/response optimization
- [ ] Memory usage optimization
- [ ] Error handling efficiency
- [ ] Connection pooling (if applicable)

#### Step 6.4: Documentation Completion
**Objective**: Complete implementation documentation

**Documentation Tasks:**
- [ ] API integration notes
- [ ] Payment method specifics
- [ ] Error handling guide
- [ ] Testing instructions
- [ ] Deployment considerations

## 🧪 Testing Strategy

### Unit Testing
- **Scope**: All transformer functions
- **Coverage**: Request/response transformations for all payment methods
- **Location**: `src/connectors/{{connector_name}}/transformers.rs`

### Integration Testing
- **Scope**: Complete gRPC flows
- **Coverage**: All implemented flows and payment methods
- **Location**: `crates/grpc-server/grpc-server/tests/{{connector_name}}_test.rs`

### Error Testing
- **Scope**: All error scenarios
- **Coverage**: Network, auth, validation, business errors
- **Approach**: Mock error responses and test handling

## 📈 Progress Tracking

### Implementation Milestones
- [ ] **Milestone 1**: Basic connector structure (Foundation)
- [ ] **Milestone 2**: Core payment flows working (Authorize, Capture, Void)
- [ ] **Milestone 3**: Refund and sync operations complete
- [ ] **Milestone 4**: Primary payment methods implemented
- [ ] **Milestone 5**: All payment methods supported
- [ ] **Milestone 6**: Advanced features complete
- [ ] **Milestone 7**: Production ready with full testing

### Progress Log
**AI must update this section after each major milestone with detailed completion notes:**

```
[MILESTONE_DATE] [✅ COMPLETED] Milestone X: Brief description
- Detailed implementation notes
- Files modified/created
- Key decisions made
- Testing results
```

### Quality Gates
- [ ] All unit tests passing
- [ ] All integration tests passing
- [ ] Error scenarios handled
- [ ] Code review completed
- [ ] Documentation updated
- [ ] Performance benchmarks met

## 🔄 Resumption Instructions

**For AI systems continuing this implementation:**

1. **Check Progress**: Review completed milestones and current implementation state
2. **Review Feedback**: Check user feedback in this file and learnings.md for improvement areas
3. **Identify Next Steps**: Use this plan to determine what to implement next
4. **Maintain Consistency**: Follow established patterns from completed work
5. **Update Documentation**: Keep implementation notes current
6. **Test Incrementally**: Ensure each addition works before proceeding
7. **Request Feedback**: Ask for optional user feedback after completing significant flows

**State Files to Check:**
- `grace-ucs/connector_integration/{{connector_name}}/{{connector_name}}_specs.md`
- `crates/integrations/connector-integration/src/connectors/{{connector_name}}.rs`
- `crates/integrations/connector-integration/src/connectors/{{connector_name}}/transformers.rs`
- `crates/grpc-server/grpc-server/tests/{{connector_name}}_test.rs`

**Common Continuation Commands:**
- `continue implementing {{connector_name}} connector in UCS - I have completed [X] and need to implement [Y]`
- `add [payment_method] to {{connector_name}} using grace/rulesbook/codegen/.gracerules_add_payment_method`
- `add [flow] flow to {{connector_name}} using grace/rulesbook/codegen/.gracerules_add_flow`
- `implement webhook handling for {{connector_name}} connector in UCS`
- `debug {{connector_name}} connector [specific_issue] in UCS`

---

This plan provides a comprehensive roadmap for UCS connector implementation that can be followed sequentially or used to resume work at any stage.
```

This implementation plan template is specifically designed for UCS architecture and provides clear guidance for:
1. **Fresh implementations** - complete step-by-step process
2. **Partial continuations** - resuming from any implementation state  
3. **Feature additions** - adding specific capabilities to existing connectors
4. **Debugging assistance** - structured approach to fixing issues

The plan emphasizes UCS-specific patterns, comprehensive payment method support, and maintains the resumable development philosophy that makes GRACE-UCS powerful for ongoing connector development.