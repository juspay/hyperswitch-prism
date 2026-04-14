# UCS Proto Service Coverage Report

**Generated:** 2026-04-03
**Branch:** cypress-test-for-ucs

---

## Executive Summary

- **Total gRPC RPC Methods (excluding ignored):** 27
- **Covered with Test Suites:** 21
- **Missing Coverage:** 6
- **Coverage Percentage:** 77.8%

**Note:** PayoutService (8 methods) and DisputeService (4 methods) are currently ignored.

---

## Detailed Coverage by Service

### ✅ CustomerService (100% - 1/1)

| Method | Status | Suite |
|--------|--------|-------|
| Create | ✓ | create_customer |

---

### ⊘ DisputeService (IGNORED)

This service is currently ignored in coverage analysis.

---

### ❌ EventService (0% - 0/1)

| Method | Status | Suite |
|--------|--------|-------|
| HandleEvent | ✗ | NO SUITE |

**Impact:** Low - Internal infrastructure
**Priority:** Low (may not need connector tests)

---

### ✅ MerchantAuthenticationService (100% - 3/3)

| Method | Status | Suite |
|--------|--------|-------|
| CreateServerAuthenticationToken | ✓ | server_authentication_token |
| CreateServerSessionAuthenticationToken | ✓ | create_session_token |
| CreateClientAuthenticationToken | ✓ | create_sdk_session_token |

---

### ✅ PaymentMethodAuthenticationService (100% - 3/3)

| Method | Status | Suite |
|--------|--------|-------|
| PreAuthenticate | ✓ | pre_authenticate |
| Authenticate | ✓ | authenticate |
| PostAuthenticate | ✓ | post_authenticate |

---

### ⚠️ PaymentMethodService (50% - 1/2)

| Method | Status | Suite |
|--------|--------|-------|
| Tokenize | ✓ | tokenize_payment_method |
| Eligibility | ✗ | NO SUITE |

**Impact:** Medium - Pre-flight checks
**Priority:** Medium

---

### ⚠️ PaymentService (69% - 11/16)

| Method | Status | Suite |
|--------|--------|-------|
| Authorize | ✓ | authorize, complete_authorize |
| Get | ✓ | get |
| Void | ✓ | void |
| Reverse | ✓ | reverse |
| Capture | ✓ | capture |
| CreateOrder | ✓ | create_order |
| Refund | ✓ | refund |
| IncrementalAuthorization | ✓ | incremental_authorization |
| VerifyRedirectResponse | ✓ | verify_redirect_response |
| SetupRecurring | ✓ | setup_recurring |
| TokenAuthorize | ✗ | NO SUITE |
| TokenSetupRecurring | ✗ | NO SUITE |
| ProxyAuthorize | ✗ | NO SUITE |
| ProxySetupRecurring | ✗ | NO SUITE |

**Missing Coverage Impact:**
- TokenAuthorize/TokenSetupRecurring: Medium priority (token-based flows)
- ProxyAuthorize/ProxySetupRecurring: Low priority (advanced use cases)

---

### ⊘ PayoutService (IGNORED)

This service is currently ignored in coverage analysis.

---

### ✅ RecurringPaymentService (100% - 2/2)

| Method | Status | Suite |
|--------|--------|-------|
| Charge | ✓ | recurring_charge |
| Revoke | ✓ | revoke_mandate |

---

### ✅ RefundService (100% - 1/1)

| Method | Status | Suite |
|--------|--------|-------|
| Get | ✓ | refund_sync |

---

## Gap Analysis

### Medium Priority Gaps

1. **Token-Based Payments (0/2 covered)**
   - TokenAuthorize, TokenSetupRecurring
   - Affects: Merchants using saved payment methods
   - Estimated effort: 1 day

2. **Payment Method Eligibility (0/1 covered)**
   - Pre-flight checks for payment methods
   - Affects: Dynamic payment method selection
   - Estimated effort: 0.5 days

### Low Priority Gaps

3. **Proxy Payments (0/2 covered)**
   - ProxyAuthorize, ProxySetupRecurring
   - Affects: Advanced routing scenarios
   - Estimated effort: 1 day

4. **Event Handling (0/1 covered)**
   - May be internal-only, not requiring connector tests
   - Verify if this needs connector integration testing

---

## Recommendations

### Immediate Actions

1. **Test Existing Suites**
   - Run all 22 existing suites against primary connectors
   - Use: `./test_suite.sh stripe`
   - Document any failures

2. **Verify Merge Success**
   - Ensure authentication suite refactoring works correctly
   - Focus on: server_authentication_token, create_session_token, create_sdk_session_token

### Short Term (1-2 weeks)

3. **Add Token Payment Suites**
   - TokenAuthorize suite
   - TokenSetupRecurring suite

### Medium Term (1-2 months)

4. **Add Payment Method Eligibility Suite**
   - Pre-flight validation testing

### Long Term

5. **Add Proxy Payment Suites** (if needed)
   - Based on feature usage analysis

---

## Testing Tools

### Coverage Checker
```bash
cargo run --bin check_coverage
```

### Test Runner
```bash
# Test all core suites for a connector
./crates/internal/integration-tests/test_suite.sh stripe

# Test specific suite
./crates/internal/integration-tests/test_suite.sh stripe authorize
```

### Manual Test
```bash
# Single suite
cargo run --bin suite_run_test -- --suite authorize --connector stripe

# Single scenario
cargo run --bin run_test -- --suite authorize --scenario no3ds_auto_capture_card --connector stripe
```

---

## Coverage Goals

### Current: 77.8% (21/27 methods, excluding PayoutService and DisputeService)

### Target Q2 2026: 92.6%
- Add token payment suites (2 methods)
- Add payment method eligibility (1 method)
- Add proxy payment suites (2 methods)
- **New coverage:** 25/27 = 92.6%

### Target Q3 2026: 96.3%
- Add EventService/HandleEvent if needed (1 method)
- **New coverage:** 26/27 = 96.3%

### Future Considerations
- **PayoutService (12 methods)** - Add when payout features are prioritized
- **DisputeService (4 methods)** - Add when dispute handling is prioritized
- EventService/HandleEvent may not require connector testing (internal-only)

---

## Appendix: Suite List

### Existing Suites (22)

1. authenticate_suite
2. authorize_suite
3. capture_suite
4. complete_authorize_suite
5. create_customer_suite
6. create_order_suite
7. create_sdk_session_token_suite
8. create_session_token_suite
9. get_suite
10. incremental_authorization_suite
11. post_authenticate_suite
12. pre_authenticate_suite
13. recurring_charge_suite
14. refund_suite
15. refund_sync_suite
16. reverse_suite
17. revoke_mandate_suite
18. server_authentication_token_suite
19. setup_recurring_suite
20. tokenize_payment_method_suite
21. verify_redirect_response_suite
22. void_suite

### Proposed New Suites (6)

**Token Payments:**
23. token_authorize_suite
24. token_setup_recurring_suite

**Other:**
25. payment_method_eligibility_suite
26. proxy_authorize_suite (optional)
27. proxy_setup_recurring_suite (optional)
28. event_handle_suite (optional - may be internal-only)

### Future Suites (Currently Ignored)

**Dispute Management (if/when needed):**
- dispute_submit_evidence_suite
- dispute_get_suite
- dispute_defend_suite
- dispute_accept_suite

**Payouts (if/when needed):**
- payout_create_suite
- payout_transfer_suite
- payout_get_suite
- payout_void_suite
- payout_stage_suite
- payout_create_link_suite
- payout_create_recipient_suite
- payout_enroll_disburse_account_suite

---

**Report Generated By:** cargo run --bin check_coverage
**Last Updated:** 2026-04-03
