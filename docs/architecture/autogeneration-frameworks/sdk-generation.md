# SDK Generation

You get idiomatic SDKs in Node.js, Python, Java, Rust, and Go without maintaining five separate codebases. Prism generates language-specific bindings from the same protobuf definitions, ensuring every SDK stays synchronized with the core API.

## The Multi-Language Problem

Payment SDKs usually force you into one language or provide thin wrappers with inconsistent patterns. A Python developer sees async/await while a Java developer sees futures. Error handling differs. Type definitions drift.

Prism solves this by generating each SDK from the protobuf source with language-specific templates. The core logic lives in one place (Rust). Each language gets bindings that feel native.

## Generation Pipeline

```
protobuf definitions → parse messages/services → language templates → SDK code
```

The generator produces:

| Component | Node.js | Python | Java | Rust | Go |
|-----------|---------|--------|------|------|-----|
| **Types** | TypeScript interfaces | dataclasses | POJOs | structs | structs |
| **Client** | Promise-based | async/await | CompletableFuture | async/await | goroutines |
| **Errors** | Error classes | Exceptions | Exceptions | Result<T,E> | error returns |
| **Builders** | Object literals | dataclass instantiation | Builder pattern | struct init | struct literals |

## FFI vs gRPC Bindings

SDKs connect to the core through two paths:

### FFI Bindings (In-Process)

The Rust core compiles to a shared library. SDKs load it via FFI:

```rust
// Rust core exports
#[no_mangle]
pub extern "C" fn authorize(
    request: *const c_char,
    response: *mut *mut c_char
) -> i32;
```

Language bindings wrap this:
- **Node.js**: `ffi-napi` with async wrappers
- **Python**: `ctypes` with type hints
- **Java**: JNI with memory management

Zero network overhead. Single process. Fastest for high-throughput applications.

### gRPC Bindings (Out-of-Process)

The core runs as a microservice. SDKs connect via gRPC:

```protobuf
service PaymentService {
  rpc Authorize(AuthorizeRequest) returns (AuthorizeResponse);
}
```

Language bindings use native gRPC clients:
- **Node.js**: `@grpc/grpc-js`
- **Python**: `grpcio`
- **Java**: `grpc-java`
- **Go**: `google.golang.org/grpc`

Better for service isolation, containerized deployments, or when you need the core as a shared service.

## Generated Code Example

Here's how the same `Authorize` call looks across languages:

**Node.js:**
```javascript
const response = await client.payments.authorize({
  amount: { minorAmount: 1000, currency: 'USD' },
  paymentMethod: { card: { cardNumber: '4242424242424242', ... } },
  captureMethod: CaptureMethod.AUTOMATIC
});
```

**Python:**
```python
response = await client.payments.authorize(
    amount=Amount(minor_amount=1000, currency="USD"),
    payment_method=PaymentMethod(card=Card(...)),
    capture_method=CaptureMethod.AUTOMATIC
)
```

**Java:**
```java
var response = client.payments()
    .authorize(AuthorizeRequest.builder()
        .amount(Amount.of(1000, Currency.USD))
        .paymentMethod(PaymentMethod.card(...))
        .captureMethod(CaptureMethod.AUTOMATIC)
        .build()
    )
    .get(); // CompletableFuture
```

Same functionality. Idiomatic patterns for each language.

## Type Safety

Generated types catch errors at compile time:

```typescript
// TypeScript: Won't compile if you miss required fields
const response = await client.payments.authorize({
  amount: { minorAmount: 1000 }, // Error: missing currency
});
```

```python
# Python: IDE shows type hints, mypy catches errors
response = client.payments.authorize(
    amount=Amount(minor_amount=1000)  # mypy: Missing required argument
)
```

## Regeneration on API Changes

When the protobuf definitions change:

1. Update proto file
2. Run `make generate-sdks`
3. All language SDKs regenerate
4. Tests verify backward compatibility

No manual edits across five languages.

## Regression Testing

Each generated SDK has regression tests generated from the same test specs:

```yaml
test: authorize_with_card
languages: [nodejs, python, java, rust, go]
request: { ... }
expect:
  status: AUTHORIZED
```

The generator creates:
- `authorize_with_card_test.js` for Node.js
- `test_authorize_with_card.py` for Python
- `AuthorizeWithCardTest.java` for Java

Same test logic. Language-specific implementation.

## Versioning

SDK versions follow the core:

| Core Version | SDK Versions |
|--------------|--------------|
| 1.2.0 | `@juspay/hyperswitch-prism@1.2.0` |
| 1.2.0 | `hyperswitch-prism==1.2.0` |
| 1.2.0 | `com.juspay:hyperswitch-prism:1.2.0` |

Patch updates auto-generate. Minor and major versions sync with core releases.

## Adding a New Language

To add support for a new language:

1. Create template files for types, client, and errors
2. Add language-specific FFI or gRPC binding generator
3. Define idiomatic patterns (builders, async, error handling)
4. Generate and test

The framework already handles protobuf parsing. You just define the language conventions.

## Benefits

- **Consistency**: Same API surface across all languages
- **Currency**: All SDKs update when protos change
- **Correctness**: Generated code passes type checks
- **Idiomatic**: Feels native to each language
- **Tested**: Regression tests verify every language

Your polyglot team uses the same payment API with their preferred patterns.
