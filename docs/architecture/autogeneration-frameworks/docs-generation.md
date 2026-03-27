# Documentation Generation

You get API reference docs that stay current without anyone manually updating markdown files every time a field changes. Prism generates all `/docs-generated` content from the source proto definitions and a rules file that enforces consistency.

## The Rules Engine

Documentation at this scale drifts. A developer adds a field to the proto, forgets to update the docs, and now the examples don't compile. Prism prevents this by treating documentation as code.

The `docs/rules/rules.md` file contains 700+ lines of specifications that define:

- Front matter format for every page
- Required sections (Overview, Purpose, Request Fields, Response Fields, Examples)
- Code example standards (test card numbers, authentication headers, endpoint format)
- Table formats for operations, field documentation, and type references
- Cross-linking patterns between related operations

When the proto definitions change, the documentation regenerates automatically. No human has to remember which pages need updates.

## Generation Pipeline

```
proto definitions → parse messages/fields → apply rules.md templates → markdown output
```

The generator reads the protobuf service definitions, extracts RPC operations and their request/response message structures, then formats them according to the rules specification.

For example, the `PaymentService/Authorize` RPC becomes:

1. **Overview section** — Business context for authorization flows
2. **Purpose section** — When to use Authorize vs CreateOrder
3. **Request Fields table** — Every field from `AuthorizeRequest` with types and descriptions
4. **Response Fields table** — Every field from `AuthorizeResponse`
5. **Example section** — A working grpcurl command using Stripe test credentials

## What Gets Generated

| Documentation Type | Source | Location |
|-------------------|--------|----------|
| Service operations | Proto RPC definitions | `/docs-generated/api-reference/services/{service}/{operation}.md` |
| Service overview | Proto service comments | `/docs-generated/api-reference/services/{service}/README.md` |
| Connector guides | Connector implementation status | `/docs-generated/connectors/{connector}.md` |
| SDK reference | Proto types + examples | `/docs-generated/sdks/{language}/README.md` |

The generation preserves hand-written content in `/docs` while keeping API reference in `/docs-generated` strictly derived from source.

## Field Documentation Rules

The rules enforce completeness. Every field from the proto appears in the generated tables:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `amount` | Money | Yes | Amount to authorize in minor units |
| `currency` | string | Yes | ISO 4217 currency code |
| `payment_method` | PaymentMethod | Yes | Card, wallet, or bank transfer details |

Missing fields in documentation are impossible because the generator pulls directly from the proto definition.

## Example Consistency

All examples follow the same pattern:

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "amount": {"minor_amount": 1000, "currency": "USD"},
    "payment_method": {
      "card": {
        "card_number": "4242424242424242",
        "expiry_month": "12",
        "expiry_year": "2027"
      }
    }
  }' \
  localhost:8080 types.PaymentService/Authorize
```

Test credentials stay consistent across all 50+ connector examples. Developers copy-paste and it works.

## Keeping Docs in Sync

When you add a new RPC to the PaymentService proto:

1. The generator creates a new markdown file at `/docs-generated/api-reference/services/payment-service/{rpc}.md`
2. The service README gets updated with a link to the new operation
3. Cross-references from related operations update automatically
4. The SUMMARY.md file includes the new page

No manual editing of individual markdown files. The rules.md specification drives everything.

## Validation

The documentation build includes a validation pass that checks:

- All proto RPCs have corresponding markdown files
- All required sections exist in each file
- Code examples use valid test credentials
- Internal links resolve correctly
- Front matter is properly formatted

Build fails if documentation doesn't match the rules. This prevents drift.

## Benefits

- **Completeness**: Every field documented, every time
- **Consistency**: Same format across 500+ pages
- **Currency**: Docs update when protos change
- **Accuracy**: Examples use real test data that compiles

Your API documentation becomes a build artifact, not a maintenance burden.
