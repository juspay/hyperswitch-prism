# Subagent Prompts — add-payment-method

Each step can be delegated to an independent subagent.

---

## Subagent 1: Analysis & Category Resolution

**Inputs**: connector_name, requested_payment_methods
**Outputs**: connector state, category mapping, implementation plan

```
Analyze the {ConnectorName} connector and resolve payment method categories.

Connector file: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs
Tech spec: grace/rulesbook/codegen/references/{connector_name}/technical_specification.md
Category mapping: .skills/add-payment-method/references/category-mapping.md

Requested payment methods: {payment_methods}

Instructions:
1. Verify the connector exists. If not → FAILED.

1a. Check tech spec exists at:
    grace/rulesbook/codegen/references/{connector_name}/technical_specification.md
    or: grace/rulesbook/codegen/references/specs/{connector_name}.md
    If missing → FAILED with reason "Tech spec not found. Run generate-tech-spec skill first."

2. Verify the Authorize flow is implemented (check create_all_prerequisites! for flow: Authorize).
   If Authorize is missing → FAILED, must add Authorize first.

3. Read the tech spec for payment-method-specific API requirements for each requested PM.

4. Map each requested payment method to its PaymentMethodData category using category-mapping.md:
   - e.g., "Apple Pay" → Wallet → PaymentMethodData::Wallet(WalletData::ApplePayThirdPartySdk)
   - e.g., "SEPA" → BankDebit → PaymentMethodData::BankDebit(BankDebitData::SepaBankDebit)

5. Identify which payment methods are already supported by reading the existing
   match arms in the Authorize TryFrom in transformers.rs.

6. Check if Refund/Capture flows exist and whether they need PM-specific changes.

Output:
  CONNECTOR: {ConnectorName}
  AUTHORIZE_EXISTS: YES | NO
  EXISTING_PMS: [Card, ...] (already implemented)
  NEW_PMS:
    - name: Apple Pay, category: Wallet, variant: WalletData::ApplePayThirdPartySdk
      pattern_file: references/payment-method-patterns/wallet.md
    - name: Google Pay, category: Wallet, variant: WalletData::GooglePay
      pattern_file: references/payment-method-patterns/wallet.md
  REFUND_NEEDS_CHANGES: YES | NO
  CAPTURE_NEEDS_CHANGES: YES | NO
  STATUS: READY | BLOCKED
```

---

## Subagent 2: Payment Method Implementation (per PM or per category)

**Inputs**: connector_name, payment_method, category, variant, pattern_file
**Outputs**: PM implemented, build passes

```
Add {PaymentMethod} ({Category}) support to the {ConnectorName} connector.

Tech spec: grace/rulesbook/codegen/references/{connector_name}/technical_specification.md
Pattern file: .skills/add-payment-method/references/payment-method-patterns/{category}.md
Connector file: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

Payment method details:
  Name: {PaymentMethod}
  Category: {Category}
  PaymentMethodData variant: {variant}

Instructions:
1. Read the category pattern file for implementation patterns.

2. Open transformers.rs, find the TryFrom implementation for the Authorize request.
   Locate the `match payment_method_data` block.

3. Add a match arm for the new payment method:
   - Extract relevant fields from the payment method data
   - Build the connector-specific request fields per the tech spec
   - For Wallet sub-variants: add nested match (ApplePayThirdPartySdk, GooglePay, etc.)
   - For Box-wrapped types (BankTransfer, GiftCard): use .deref()

4. Handle unsupported variants:
   - Never use catch-all _ silently
   - Return ConnectorError::NotImplemented with specific message including connector name

5. Validate required fields with missing_field_err.

6. If Refund/Capture flows have payment_method_data match blocks, add arms there too.

7. Run: cargo build --package connector-integration
8. Fix compilation errors.

Output:
  PAYMENT_METHOD: {PaymentMethod}
  STATUS: SUCCESS | FAILED
  BUILD: PASS | FAIL
  FILES_MODIFIED: [transformers.rs]
  REASON: (if failed)
```

---

## Subagent 3: gRPC Testing

**Inputs**: connector_name, payment_methods_to_test
**Outputs**: test results per payment method

```
Test the {ConnectorName} connector with newly added payment methods via grpcurl.

Testing guide: .skills/add-payment-method/references/grpc-testing-guide.md
Credentials: creds.json (field: {connector_name})
Connector file: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

Payment methods to test: {pm_list}

Instructions:
1. Read the testing guide for grpcurl templates
2. Start the gRPC server if not running
3. Load credentials from creds.json
4. For each payment method, run the Authorize grpcurl test with the correct
   payment_method object. Examples:

   Card:
     "payment_method": { "card": { "card_number": {"value": "4111..."}, ... } }

   Apple Pay:
     "payment_method": { "wallet": { "apple_pay_third_party_sdk": { "payment_data": "..." } } }

   Google Pay:
     "payment_method": { "wallet": { "google_pay": { "tokenization_data": {"token": "..."} } } }

   UPI:
     "payment_method": { "upi": { "upi_collect": { "vpa_id": {"value": "test@upi"} } } }

   Bank Debit (ACH):
     "payment_method": { "bank_debit": { "ach": { "account_number": {"value": "..."}, ... } } }

5. Validate response against PASS/FAIL criteria
6. If FAILED: read server logs, fix code, rebuild, retest (max 7 iterations)

Output:
  CONNECTOR: {ConnectorName}
  RESULTS:
    {PaymentMethod}: PASS | FAIL
    ...
  STATUS: ALL_PASS | PARTIAL | ALL_FAIL
```

---

## Subagent 4: Quality Review

**Inputs**: connector_name
**Outputs**: violations list, pass/fail

```
Quality review the {ConnectorName} connector after adding payment methods.

Quality checklist: .skills/add-payment-method/references/quality-checklist.md (if exists)
Connector file: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

Checks:
1. Each supported payment method has its own explicit match arm
2. Unsupported variants return ConnectorError::NotImplemented with connector name
3. No catch-all _ silently drops payment methods without error
4. Required fields validated with missing_field_err or ok_or_else
5. Box-wrapped types (BankTransferData, GiftCardData) properly dereferenced with .deref()
6. No unwrap() calls
7. Naming and formatting consistent with existing code
8. cargo build --package connector-integration passes

Output:
  CONNECTOR: {ConnectorName}
  VIOLATIONS: [list] or [none]
  STATUS: PASS | FAIL
```
