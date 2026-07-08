# Integration Tests - Testing Plan & Coverage Analysis

## Overview

This document outlines the testing strategy for the integration test suite and identifies coverage gaps for gRPC proto services.

## Coverage Summary

**Total proto RPC methods:** 39
**Covered with suites:** 18
**Missing coverage:** 21
**Coverage percentage:** 46.2%

---

## 1. Suite Testing Strategy

### Priority 1: Critical Payment Flows (COVERED ✓)
These are the most commonly used payment flows and should be tested first:

1. **authorize_suite** → `PaymentService/Authorize`
   - Test with multiple connectors: stripe, adyen, checkout
   - Verify card, bank_redirect, wallet payment methods
   - Check 3DS and non-3DS scenarios

2. **capture_suite** → `PaymentService/Capture`
   - Test manual capture after authorization
   - Verify partial captures
   - Test capture failures

3. **refund_suite** → `PaymentService/Refund`
   - Full and partial refunds
   - Multiple refunds for same payment
   - Refund timing edge cases

4. **void_suite** → `PaymentService/Void`
   - Void before capture
   - Void timing constraints
   - Multiple void attempts

### Priority 2: Authentication & Setup (COVERED ✓)
Essential for establishing sessions and customer contexts:

5. **server_authentication_token_suite** → `MerchantAuthenticationService/CreateServerAuthenticationToken`
   - Verify token creation
   - Test token expiration
   - Multi-connector support

6. **create_session_token_suite** → `MerchantAuthenticationService/CreateServerSessionAuthenticationToken`
   - Session establishment for client SDKs
   - Token format validation

7. **create_sdk_session_token_suite** → `MerchantAuthenticationService/CreateClientAuthenticationToken`
   - Client-side authentication tokens
   - Mobile SDK compatibility

8. **create_customer_suite** → `CustomerService/Create`
   - Customer profile creation
   - Duplicate handling
   - Metadata storage

### Priority 3: Advanced Payment Features (COVERED ✓)

9. **setup_recurring_suite** → `PaymentService/SetupRecurring`
   - Mandate/subscription setup
   - Verify connector mandate support

10. **recurring_charge_suite** → `RecurringPaymentService/Charge`
    - Charge against existing mandate
    - Retry logic for failed recurring charges

11. **revoke_mandate_suite** → `RecurringPaymentService/Revoke`
    - Cancel recurring mandates
    - Cleanup verification

12. **create_order_suite** → `PaymentService/CreateOrder`
    - Order-based payment flows
    - Order metadata handling

13. **incremental_authorization_suite** → `PaymentService/IncrementalAuthorization`
    - Increase authorization amount
    - Connector-specific behavior

14. **reverse_suite** → `PaymentService/Reverse`
    - Authorization reversal
    - Timing constraints

### Priority 4: 3DS Authentication (COVERED ✓)

15. **pre_authenticate_suite** → `PaymentMethodAuthenticationService/PreAuthenticate`
    - Initiate 3DS flow
    - Challenge request handling

16. **authenticate_suite** → `PaymentMethodAuthenticationService/Authenticate`
    - Complete 3DS authentication
    - Verify authentication results

17. **post_authenticate_suite** → `PaymentMethodAuthenticationService/PostAuthenticate`
    - Post-authentication data collection
    - Result finalization

### Priority 5: Additional Payment Operations (COVERED ✓)

18. **get_suite** → `PaymentService/Get`
    - Retrieve payment status
    - Payment details validation

19. **refund_sync_suite** → `RefundService/Get`
    - Refund status polling
    - Webhook vs polling comparison

20. **tokenize_payment_method_suite** → `PaymentMethodService/Tokenize`
    - Payment method tokenization
    - Token reuse scenarios

21. **verify_redirect_response_suite** → `PaymentService/VerifyRedirectResponse`
    - Return from redirect flows
    - Query parameter validation

22. **complete_authorize_suite** → `PaymentService/Authorize`
    - Complete authorization after redirect
    - 3DS result integration

---

## 2. Missing Suite Coverage

**Note:** PayoutService and DisputeService are currently ignored in coverage analysis.

### Medium Priority Missing Suites

#### Token-Based Payments (0/2 methods covered)
**Impact:** Medium - Alternative payment initiation methods

- [ ] **PaymentService/TokenAuthorize** - Authorize using saved token
- [ ] **PaymentService/TokenSetupRecurring** - Setup recurring with token

**Suggested Suite Names:**
- `token_authorize_suite`
- `token_setup_recurring_suite`

#### Proxy Payments (0/2 methods covered)
**Impact:** Low - Advanced feature for specific use cases

- [ ] **PaymentService/ProxyAuthorize** - Proxy authorization
- [ ] **PaymentService/ProxySetupRecurring** - Proxy recurring setup

**Suggested Suite Names:**
- `proxy_authorize_suite`
- `proxy_setup_recurring_suite`

#### Payment Method Eligibility (0/1 methods covered)
**Impact:** Medium - Pre-flight checks for payment methods

- [ ] **PaymentMethodService/Eligibility** - Check payment method eligibility

**Suggested Suite Name:**
- `payment_method_eligibility_suite`

#### Event Handling (0/1 methods covered)
**Impact:** Low - Webhook/event processing (may be internal-only)

- [ ] **EventService/HandleEvent** - Process incoming events

**Note:** This may be internal infrastructure only and not require connector testing.

### Future Consideration (Currently Ignored)

#### Dispute Management (0/4 methods covered)
**Status:** Currently ignored - add when dispute handling is prioritized

- **DisputeService/SubmitEvidence** - Upload evidence for dispute
- **DisputeService/Get** - Retrieve dispute details
- **DisputeService/Defend** - Initiate dispute defense
- **DisputeService/Accept** - Accept dispute outcome

#### Payout Operations (0/8 methods covered)
**Status:** Currently ignored - add when payout features are prioritized

- **PayoutService/Create** - Create payout request
- **PayoutService/Transfer** - Execute payout transfer
- **PayoutService/Get** - Get payout status
- **PayoutService/Void** - Cancel payout
- **PayoutService/Stage** - Stage payout for batch processing
- **PayoutService/CreateLink** - Generate payout link
- **PayoutService/CreateRecipient** - Register payout recipient
- **PayoutService/EnrollDisburseAccount** - Enroll disbursement account

---

## 3. Test Execution Plan

### Phase 1: Verify Existing Suites (Priority 1-5)

Run each suite against at least 3 major connectors to ensure they work after the merge:

```bash
# Test command template
cargo run --bin suite_run_test -- \
  --suite <suite_name> \
  --connector <connector_name>

# Example: Test authorize suite with stripe
cargo run --bin suite_run_test -- \
  --suite authorize \
  --connector stripe
```

**Recommended connectors for testing:**
1. **stripe** - Most complete implementation
2. **adyen** - Major processor with full feature set
3. **checkout** - Alternative major processor

**Test Matrix:**

| Suite | Stripe | Adyen | Checkout | Notes |
|-------|--------|-------|----------|-------|
| authorize | ✓ | ✓ | ✓ | Core flow |
| capture | ✓ | ✓ | ✓ | |
| refund | ✓ | ✓ | ✓ | |
| void | ✓ | ✓ | ✓ | |
| server_authentication_token | ✓ | ✓ | ✓ | Critical auth |
| create_customer | ✓ | ✓ | ✓ | |
| setup_recurring | ✓ | ✓ | - | Check connector support |
| recurring_charge | ✓ | ✓ | - | Depends on setup |
| tokenize_payment_method | ✓ | ✓ | ✓ | |
| pre_authenticate | ✓ | ✓ | - | 3DS support |
| authenticate | ✓ | ✓ | - | 3DS support |
| post_authenticate | ✓ | ✓ | - | 3DS support |
| get | ✓ | ✓ | ✓ | Status retrieval |
| refund_sync | ✓ | ✓ | ✓ | |
| create_order | ✓ | - | - | Limited support |
| incremental_authorization | ✓ | - | - | Limited support |
| reverse | ✓ | - | - | Limited support |
| verify_redirect_response | ✓ | ✓ | ✓ | |
| complete_authorize | ✓ | ✓ | ✓ | |
| revoke_mandate | ✓ | ✓ | - | |
| create_session_token | ✓ | ✓ | ✓ | |
| create_sdk_session_token | ✓ | ✓ | ✓ | |

### Phase 2: Create Missing Suites (Future Work)

Priority order for new suite creation:

1. **Token payments** (common use case)
2. **Payment method eligibility** (pre-flight checks)
3. **Proxy payments** (advanced use cases)
4. **Event handling** (if connector testing is needed)

Future priorities (currently ignored):
- **Disputes** (when dispute handling is prioritized)
- **Payouts** (when payout features are prioritized)

---

## 4. Automated Testing Scripts

### Check Proto Service Coverage

```bash
cargo run --bin check_coverage
```

This tool analyzes which gRPC proto services have test suite coverage.
Note: PayoutService and DisputeService are ignored by default.

### Run All Suites for a Connector

```bash
# From project root
./crates/internal/integration-tests/test_suite.sh stripe

# Test specific suite
./crates/internal/integration-tests/test_suite.sh stripe authorize
```

---

## 5. Success Criteria

A suite is considered "passing" if:

1. **Compilation:** All binaries compile without errors
2. **Execution:** Suite runs to completion without panics
3. **Assertions:** All scenario assertions pass
4. **Report:** Valid `report.json` generated with correct structure
5. **Display Names:** Scenario display names appear in report

---

## 6. Known Issues & Limitations

1. **Authentication suite mapping:** The coverage checker shows authentication suites as "without proto mapping" due to multi-line formatting in the Rust code. These are actually covered.

2. **Connector support variations:** Not all connectors support all flows. The test matrix should be updated based on connector capabilities.

3. **Credentials required:** Tests require valid connector credentials in `creds.json`.

4. **Test environment:** Some tests may require specific merchant configurations on the connector side.

---

## 7. Next Steps

1. ✅ Merge main branch into test branch
2. ✅ Fix all compilation errors
3. ✅ Create coverage analysis tool
4. ⏳ Execute Phase 1 testing (verify existing suites)
5. ⏳ Document test results
6. ⏳ Plan Phase 2 (new suite creation)

---

## Appendix: Running Individual Tests

### Single Suite, Single Connector
```bash
cargo run --bin suite_run_test -- --suite authorize --connector stripe
```

### Single Scenario
```bash
cargo run --bin run_test -- --suite authorize --scenario no3ds_auto_capture_card --connector stripe
```

### With SDK Backend
```bash
cargo run --bin sdk_run_test -- --suite authorize --connector stripe
```

### Generate Report
```bash
# Report is automatically generated at:
# crates/internal/integration-tests/test_report/report_structure.json
```
