# Test Generation

You get regression tests for every connector without writing them by hand. Prism generates test suites from the proto definitions and a declarative test spec, then runs them against live sandboxes to catch breaking changes before they hit production.

## Why Generate Tests

Manual test maintenance doesn't scale. With 50+ connectors, each supporting 10-20 operations, you'd need thousands of test cases. When Stripe changes their API response format or Adyen deprecates a field, manually updating every affected test takes weeks.

Generated tests solve this by deriving test cases from the source of truth: the protobuf definitions and a connector-specific test specification.

## Test Generation Framework

The framework has three layers:

| Layer | Purpose | Input | Output |
|-------|---------|-------|--------|
| **Spec Parser** | Read test definitions | Test spec YAML + proto definitions | Internal test model |
| **Generator** | Create test code | Test model + language templates | Rust/JavaScript/Python test files |
| **Runner** | Execute and validate | Generated tests + sandbox credentials | Pass/fail results with diffs |

## Test Specification Format

Tests are declared, not written. A test spec for Stripe authorization looks like:

```yaml
test: authorize_success
connector: stripe
service: PaymentService
operation: Authorize
request:
  amount:
    minor_amount: 1000
    currency: USD
  payment_method:
    card:
      card_number: "4242424242424242"
      expiry_month: "12"
      expiry_year: "2027"
expect:
  status: AUTHORIZED
  response_fields:
    payment_id: present
    connector_response: present
```

The generator turns this into a compiled test that executes the gRPC call and validates the response structure.

## What Gets Generated

### gRPC Integration Tests

Tests the full stack: SDK → gRPC → Connector Adapter → Payment Provider → Response transformation

```rust
#[tokio::test]
async fn stripe_authorize_success() {
    let client = TestClient::new().await;
    let response = client
        .payment_service()
        .authorize(stripe_test_request())
        .await
        .expect("authorize should succeed");
    
    assert_eq!(response.status, PaymentStatus::Authorized);
    assert!(response.payment_id.value.len() > 0);
}
```

### SDK Language Tests

Each SDK gets language-specific tests verifying:
- Type serialization matches proto
- Error handling follows language conventions
- Async/promise patterns work correctly

```javascript
test('Node.js SDK - authorize with card', async () => {
  const response = await client.payments.authorize({
    amount: { minorAmount: 1000, currency: 'USD' },
    paymentMethod: { card: testCard }
  });
  expect(response.status).toBe('AUTHORIZED');
});
```

### Regression Test Suites

When you add a new connector, the generator creates:
- A baseline test for each supported operation
- Error case tests (invalid credentials, expired cards)
- Webhook payload validation tests

## Running Generated Tests

```bash
# Generate tests from specs
make generate-tests

# Run against sandboxes
STRIPE_API_KEY=$TEST_KEY make test-connectors

# Run SDK tests across all languages
make test-sdks
```

Tests run against live sandbox environments, not mocks. This catches real integration issues: authentication changes, rate limiting behaviors, response format drift.

## Test Coverage

Generated tests cover:

| Test Type | Coverage |
|-----------|----------|
| Happy path | Every operation for every connector |
| Error cases | Invalid auth, declined cards, network timeouts |
| Field validation | Required fields, type constraints, enum values |
| Response mapping | Connector-specific responses transform correctly |
| Webhooks | Event parsing and signature verification |

## Updating Tests When APIs Change

When Adyen changes their response format:

1. Update the connector adapter code
2. Regenerate tests: `make generate-tests`
3. Run against sandbox: `make test-connectors`
4. Review failures, fix adapter, regenerate

The test specs stay the same. Only the generated test code and adapter logic change.

## CI Integration

Generated tests run on every PR:

```yaml
- name: Run Connector Tests
  run: make test-connectors
  env:
    STRIPE_API_KEY: ${{ secrets.STRIPE_TEST_KEY }}
    ADYEN_API_KEY: ${{ secrets.ADYEN_TEST_KEY }}
```

A connector test failure blocks merge. This prevents shipping broken integrations.

## Benefits

- **Coverage**: Every operation tested for every connector
- **Maintenance**: Update specs once, regenerate everywhere
- **Currency**: Tests stay current with proto changes
- **Confidence**: Live sandbox testing catches real issues

Your test suite scales with your connector count without scaling your test maintenance burden.
