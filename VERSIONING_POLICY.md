# Versioning Policy

**Principle**: Same `MAJOR.MINOR` = same protocol compatibility across all languages Semantic Versioning.

| Version | Rule |
|---------|------|
| **MAJOR** | Breaking change - requires code modification |
| **MINOR** | New capability - additive only, no changes needed |
| **PATCH** | Fix/security - safe to auto-update |

---

## MAJOR Changes (Breaking)

Changes that require consumers to modify their code.

| Category | Change | Example | Why It Breaks |
|----------|--------|---------|---------------|
| **Fields** | Remove field | Delete `customer_email` | Compile/runtime error |
| | Make optional required | `browser_info: Option<T>` → `T` | Existing requests rejected |
| | Change field type | `amount: i64` → `Money` | Type mismatch |
| | Change format | `txn_12345` → `txn_12345_abc` | Validation fails |
| | Change timestamp format | ISO8601 → Unix epoch | Parsing fails |
| | Required nested field | `address.city` mandatory | Valid objects rejected |
| **Functions** | Add required parameter | `fn auth(req, id: &str)` | Missing argument error |
| | Remove parameter | Remove `options` from signature | Too many arguments |
| | Reorder parameters | `auth(a, b)` → `auth(b, a)` | Wrong argument bugs |
| | Change parameter type | `amount: u64` → `i64` | Type mismatch |
| **Enums** | Remove variant | Remove `STRIPE` from `Connector` | Match arm missing |
| | Reorder/renumber | Change underlying values | Logic bugs |
| **Behavior** | Change default value | `AUTO` → `MANUAL` | Different behavior |
| | Change default | `timeout: 30s` → `5s` | Operations fail |
| | Change pagination | `50` records → `20` | Data assumptions wrong |
| | Change array order | newest-first → oldest-first | Index assumptions wrong |
| | Change null semantics | "same as billing" → "no address" | Business logic changes |
| | Change boolean default | `skip_validation: false` → `true` | Security changes |
| **Infrastructure** | Drop platform support | Remove Windows x86 | Build fails |
| | Change rate limits | 100/min → 60/min | Throughput assumptions |
| | Add required header | Mandate `X-Idempotency-Key` | Requests rejected |
| | Change wire encoding | gzip → brotli only | Protocol incompatibility |
| | Change compression | Remove gzip support | Old clients fail |
| | Add auto-retry | With exponential backoff | Idempotency issues |
| | Change retry behavior | New default retry logic | Unexpected side effects |
| **Validation** | Stricter input rules | 3-char min for IDs | Valid inputs rejected |
| | New required auth | OAuth scope required | Previously valid tokens fail |

---

## MINOR Changes (Additive)

Changes that add capabilities without requiring code changes.

| Category | Change | Implementation | Notes |
|----------|--------|----------------|-------|
| **Fields** | Add optional field | `metadata: Option<T>` | Omitted uses default |
| | Add optional nested | `address.zone: Option<String>` | Safe nesting |
| | Deprecate field | Mark `#[deprecated]` | Still works, warns |
| | Add reserved fields | `reserved 100, 101` | Prevents reuse |
| **Functions** | Add optional parameter | `idempotency_key = None` | Old calls work |
| | Add new method | `batch_authorize()` | Old code ignores it |
| | Add overload | New parameter combination | Java/Kotlin |
| **Enums** | Add variant | `TRUSTLY` to `Connector` | ⚠️ Rust needs `_ =>` |
| | Add event type | `DISPUTE_CREATED` | ⚠️ Rust needs `_ =>` |
| | Add status code | `PARTIALLY_REFUNDED` | ⚠️ Rust needs `_ =>` |
| | Add currency | `VES` Venezuelan bolívar | Safe addition |
| **Messages** | Add new RPC | `PaymentService.BatchRefund` | Independent endpoint |
| | Add new message type | `BatchRefundRequest` | New struct |
| **Infrastructure** | Add compression | brotli (keep gzip) | Choice preserved |
| | Add region endpoint | `eu.prism.api` | Additional option |
| | Add SDK language | New Go SDK | Doesn't affect others |
| **Errors** | Split error code | Parent + sub-code | Keep parent working |
| | Add error detail | More context in message | Human readable |
| **Features** | New payment method | Samsung Pay support | New enum value |
| | New connector | Peach Payments | New enum value |
| | New flow | Payout support | New RPCs |

### Rust-Specific: Enum Matching

**Problem**: Rust requires exhaustive match. New enum variants break compilation.

```rust
// ❌ BREAKS when new variant added
match connector {
    Connector::Stripe => handle_stripe(),
    Connector::Adyen => handle_adyen(),
}

// ✅ SAFE with fallback
match connector {
    Connector::Stripe => handle_stripe(),
    Connector::Adyen => handle_adyen(),
    _ => handle_default(), // Required!
}
```

**Solutions**:

| Approach | Effort | Trade-off |
|----------|--------|-----------|
| Require `_ =>` | None | Burden on users |
| `#[non_exhaustive]` wrapper | Medium | New types |
| Helper methods | High | Less flexible |

**Recommended wrapper**:
```rust
#[non_exhaustive]
pub enum ConnectorExt {
    Known(Connector),
    Unknown(i32),
}
```

---

## PATCH Changes (Fixes)

Changes that fix bugs or improve performance without changing behavior.

| Category | Change | Example |
|----------|--------|---------|
| **Security** | Fix vulnerability | Webhook signature verification |
| | Patch auth bypass | OAuth token validation |
| | Fix encryption | TLS cipher suite |
| **Bugs** | Fix parsing | Stripe error code mapping |
| | Fix calculation | Tax amount rounding |
| | Fix validation | Accept valid card formats |
| | Fix timeout | Proper deadline handling |
| | Fix memory leak | Connection pool cleanup |
| | Fix race condition | Concurrent request handling |
| **Performance** | Optimize serialization | Faster protobuf encode |
| | Reduce latency | Connection reuse |
| | Lower memory | Efficient buffering |
| | Improve throughput | Better batching |
| **Reliability** | Better retries | Exponential backoff fix |
| | Circuit breaker | Prevent cascade failures |
| | Health checks | More accurate status |
| **Documentation** | Fix typos | Error message clarity |
| | Clarify behavior | API doc updates |
| | Add examples | Usage patterns |
| **Cosmetic** | Error message text | `"Card declined"` → `"Payment declined"` |
| | Logging format | Structured log fields |
| **Permissions** | More permissive | Remove OAuth scope requirement |

**Warning**: Changing error message **structure** (adding machine-readable codes) can be breaking if consumers parse strings.

---

## Decision Framework

### Binary Questions

| Question | If YES → | If NO → |
|----------|----------|---------|
| Does old code still compile/run? | Check semantics | **MAJOR** |
| Is change purely additive? | **MINOR** | Investigate |
| Does it reject previously valid input? | **MAJOR** | - |
| Does it require new mandatory parameters? | **MAJOR** | - |
| Same code → different results? | **MAJOR** | - |
| Is it internal fix only? | **PATCH** | - |
| Just docs/logging/text? | **PATCH** | - |

### Language Behavior Matrix

| Scenario | JS/Python | Java | Rust |
|----------|-----------|------|------|
| New optional field | ✅ Ignored | ✅ Default | ✅ Default |
| Remove field | ❌ Undefined | ❌ Compile | ❌ Compile |
| New enum variant | ✅ Runtime | ✅ Default | ❌ Compile* |
| Change default | ❌ Behavior | ❌ Behavior | ❌ Behavior |
| Add optional param | ✅ Works | ✅ Works | ✅ Works |
| Add required param | ❌ Error | ❌ Compile | ❌ Compile |

*Unless using `_ =>` fallback

---

## Proto Schema Rules

### DO (Backward Compatible)
- Add new **optional** fields with new field numbers
- Add new enum variants  
- Add new RPCs/messages
- Mark deleted fields as `reserved`
- Use `deprecated` option

### DON'T (Breaking)
- Remove fields without reservation
- Change field numbers or types
- Add new **required** fields
- Change default values
- Reuse reserved field numbers

```protobuf
message PaymentRequest {
    reserved 100, 101;
    reserved "legacy_field";
    
    string merchant_id = 1;
    Money amount = 2;
    // metadata = 3;  // NEW: optional, safe
}
```

---

## Version Pinning

| Language | Pin Pattern | Effect |
|----------|-------------|--------|
| JavaScript | `"1.3.*"` | Patch + Minor |
| Python | `"~=1.3.0"` | Patch + Minor |
| Java | `"[1.3.0,1.4.0)"` | Patch only |
| Rust | `"1.3.*"` | Patch + Minor* |

*Rust: Use `_ =>` fallbacks

---

## Deprecation Policy

1. **vN.x.x** (MINOR): Deprecate, add warnings
2. **vN.x.x** (MINOR): Document migration path
3. **v(N+1).0.0** (MAJOR): Remove after 6+ months

---

## References

- [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html)
- [Rust RFC 2008: `#[non_exhaustive]`](https://github.com/rust-lang/rfcs/blob/master/text/2008-non-exhaustive.md)
- [Protocol Buffers Compatibility](https://github.com/protocolbuffers/protobuf/blob/3.9.x/CHANGES.txt)
