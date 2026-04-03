# Code Generation

You get a working connector adapter in hours instead of weeks. Prism uses Grace, a code generation tool that reads payment provider API specs and produces Rust connector integration code with proper request/response transformations.

## The Problem with Manual Integration

Writing a connector adapter requires understanding:
- The payment provider's authentication scheme
- How their API maps to unified types (amounts, currencies, payment methods)
- Error code mappings
- Webhook payload structures
- Testing patterns

For a typical connector like Stripe or Adyen, this is 2,000-5,000 lines of Rust code. Done manually, it takes weeks and introduces bugs. Grace automates the repetitive 80% so developers focus on the interesting 20%.

## Grace Architecture

Grace has two interfaces: a CLI tool and a skill/prompt system for LLMs.

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   API Spec      │────▶│  Grace Parser    │────▶│  Rust Adapter   │
│ (OpenAPI/JSON)  │     │  + Templates     │     │  Code           │
└─────────────────┘     └──────────────────┘     └─────────────────┘
         │                       │                        │
         ▼                       ▼                        ▼
   Provider docs           LLM Skill              Connector-specific
   and examples           augmentation             business logic
```

## CLI Usage

Generate a connector scaffold from an OpenAPI spec:

```bash
# Generate from OpenAPI spec
grace generate \
  --spec ./adyen-openapi.json \
  --connector adyen \
  --output ./crates/integrations/connector-integration/src/connectors/adyen/

# Generate with custom LLM model
grace generate \
  --spec ./adyen-openapi.json \
  --connector adyen \
  --model gpt-4 \
  --api-key $OPENAI_API_KEY \
  --output ./crates/integrations/connector-integration/src/connectors/adyen/
```

The CLI produces:
- `connector.rs` — The adapter struct and trait implementation
- `transformers.rs` — Request/response mapping functions
- `types.rs` — Connector-specific type definitions
- `test.rs` — Generated test scaffolding

## LLM Skill Integration

Grace includes a skill definition that any LLM can use. Connect your own model:

```bash
# Start Grace with custom model endpoint
grace server \
  --model-endpoint https://api.anthropic.com/v1/messages \
  --model claude-3-opus-20240229 \
  --api-key $ANTHROPIC_API_KEY

# Use via the skill
grace skill generate-connector \
  --spec ./provider-api.json \
  --name "new-provider"
```

The skill prompt includes:
- Prism's unified type system
- Common transformation patterns
- Error mapping conventions
- Rust code templates

Your LLM generates code that follows Prism conventions without training on proprietary code.

## What Gets Generated

### Request Transformers

```rust
impl TryFrom<AuthorizeRequest> for AdyenPaymentRequest {
    type Error = IntegrationError;
    
    fn try_from(req: AuthorizeRequest) -> Result<Self, Self::Error> {
        Ok(AdyenPaymentRequest {
            amount: req.amount.minor_amount,
            currency: req.amount.currency.to_string(),
            payment_method: req.payment_method.try_into()?,
            reference: req.merchant_order_id,
            // ... additional fields
        })
    }
}
```

### Response Transformers

```rust
impl TryFrom<AdyenPaymentResponse> for AuthorizeResponse {
    type Error = ConnectorResponseTransformationError;
    
    fn try_from(resp: AdyenPaymentResponse) -> Result<Self, Self::Error> {
        Ok(AuthorizeResponse {
            payment_id: resp.psp_reference.into(),
            status: resp.result_code.into(),
            amount: resp.amount.try_into()?,
            // ... additional fields
        })
    }
}
```

### Error Mapping

```rust
impl From<AdyenErrorCode> for UnifiedError {
    fn from(code: AdyenErrorCode) -> Self {
        match code {
            AdyenErrorCode::Refused => UnifiedError::PaymentDeclined,
            AdyenErrorCode::ExpiredCard => UnifiedError::ExpiredCard,
            AdyenErrorCode::InvalidCardNumber => UnifiedError::InvalidCard,
            // ... additional mappings
        }
    }
}
```

## Customization Points

Generated code includes `TODO` markers for connector-specific logic:

```rust
fn authenticate(&self, creds: &ConnectorCredentials) -> Result<AuthHeader, Error> {
    // TODO: Implement authentication for this connector
    // Most providers use API key in header, some use OAuth
    todo!("Implement authentication")
}
```

You fill in the blanks. The boilerplate structure is done.

## Validation

Grace validates generated code:
- Type checks against Prism interfaces
- Serialization roundtrips (unified → connector → unified)
- Required field coverage
- Error case handling

```bash
# Validate generated connector
grace validate \
  --connector ./crates/integrations/connector-integration/src/connectors/adyen/
```

## Adding a New Connector

```bash
# 1. Obtain API spec from provider
# 2. Generate scaffold
grace generate --spec ./spec.json --connector new-provider --output ./connectors/

# 3. Implement TODOs (authentication, special cases)
# 4. Validate
grace validate --connector ./connectors/new-provider/

# 5. Run tests
make test-connector CONNECTOR=new-provider
```

A basic connector adapter takes 1-2 days instead of 2-3 weeks.

## Benefits

- **Speed**: Days instead of weeks for new connectors
- **Consistency**: All adapters follow the same patterns
- **Correctness**: Generated code passes type checks and validation
- **Maintainability**: Update the spec, regenerate the code
- **Flexibility**: Bring your own LLM, no vendor lock-in

Connector integration becomes configuration, not implementation.
