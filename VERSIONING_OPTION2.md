# Versioning Policy - Option 2: Enhanced Separate Tables (RECOMMENDED)

> **Note**: This is the RECOMMENDED approach. Three focused tables organized by severity (MAJOR, MINOR, PATCH) with language impact columns where relevant. Easier to scan and maintain than Option 1.

**Principle**: Same `MAJOR.MINOR` = same protocol compatibility across all languages.

| Version | Rule |
|---------|------|
| **MAJOR** | Breaking change - requires code modification |
| **MINOR** | New capability - additive only, no changes needed |
| **PATCH** | Fix/security - safe to auto-update |

---

## MAJOR Changes (Breaking)

Changes that require consumers to modify their code.

| Category | Change | Why It Breaks | JS/Python | Java | Rust |
|----------|--------|---------------|-----------|------|------|
| **Fields** | Remove field | Code referencing field breaks | ❌ undefined | ❌ NPE | ❌ Compile error |
| **Fields** | Make optional required | Existing requests rejected | ❌ Validation | ❌ Validation | ❌ Compile error |
| **Fields** | Change field type | Type mismatch | ❌ TypeError | ❌ ClassCast | ❌ Type mismatch |
| **Fields** | Change format | Validation fails | ❌ Parse error | ❌ Parse error | ❌ Parse error |
| **Fields** | Change timestamp format | Date parsing fails | ❌ Date error | ❌ Date error | ❌ Parse error |
| **Fields** | Change ID format | Length/prefix check fails | ❌ Validation | ❌ Validation | ❌ Validation |
| **Fields** | Required nested field | Objects missing field rejected | ❌ Validation | ❌ Validation | ❌ Compile error |
| **Functions** | Add required parameter | Missing argument | ❌ TypeError | ❌ Compile | ❌ Compile error |
| **Functions** | Remove parameter | Calls passing param fail | ❌ Wrong args | ❌ Compile | ❌ Compile error |
| **Functions** | Reorder parameters | Wrong argument binding | ❌ Silent bug | ❌ Compile | ❌ Compile error |
| **Functions** | Change parameter type | Argument rejected | ❌ Coercion fail | ❌ Compile | ❌ Type mismatch |
| **Functions** | Rename parameter (named) | Keyword lookup fails | ❌ KeyError | ❌ Named arg | ✅ Works |
| **Functions** | Change return type | Destructuring breaks | ❌ Property fail | ❌ Cast fail | ❌ Type mismatch |
| **Functions** | Remove return field | Field access fails | ❌ undefined | ❌ Null | ❌ Compile error |
| **Methods** | Rename method | Method not found | ❌ AttributeError | ❌ Compile | ❌ Compile error |
| **Methods** | Remove method | Method not found | ❌ AttributeError | ❌ Compile | ❌ Compile error |
| **Methods** | Change visibility | Access denied | ❌ Access denied | ❌ Access denied | ❌ Compile error |
| **Enums** | Remove variant | Match arm missing | ❌ undefined | ❌ IllegalArg | ❌ Compile error |
| **Enums** | Change value | Numeric mapping wrong | ❌ Logic error | ❌ Logic error | ❌ Logic error |
| **Enums** | Change semantics | State machine breaks | ❌ Logic error | ❌ Logic error | ❌ Logic error |
| **Constructors** | Remove no-arg constructor | Default construction breaks | ❌ Instantiate fail | ❌ Compile | ❌ Compile error |
| **Constructors** | Make param required | Missing argument | ❌ Instantiate fail | ❌ Compile | ❌ Compile error |
| **Constructors** | Remove builder method | Fluent API breaks | ❌ Chain fails | ❌ Compile | ❌ Compile error |
| **Config** | Remove config option | Setting missing | ❌ KeyError | ❌ Null/Error | ❌ Compile error |
| **Config** | Change default value | Different outcome | ❌ Behavior | ❌ Behavior | ❌ Behavior |
| **Config** | Change format | Parser fails | ❌ Parse error | ❌ Parse error | ❌ Parse error |
| **Behavior** | Change default action | Same code, different result | ❌ Behavior | ❌ Behavior | ❌ Behavior |
| **Behavior** | Change pagination | Data assumptions break | ❌ Assumptions | ❌ Assumptions | ❌ Assumptions |
| **Behavior** | Change sort order | Ordering changed | ❌ Wrong order | ❌ Wrong order | ❌ Wrong order |
| **Behavior** | Change null semantics | Null means different | ❌ Logic error | ❌ Logic error | ❌ Logic error |
| **Errors** | Remove error variant | Catch/match fails | ❌ Except fail | ❌ Catch fail | ❌ Match incomplete |
| **Errors** | Change error code | String matching fails | ❌ Match fail | ❌ Match fail | ❌ Match fail |
| **Errors** | Change hierarchy | instanceof fails | ❌ isinstance | ❌ instanceof | ❌ Downcast fail |
| **Infrastructure** | Drop platform support | Build fails | ❌ Import error | ❌ ClassNotFound | ❌ Link error |
| **Infrastructure** | Change rate limits | Throttled | ❌ 429 errors | ❌ 429 errors | ❌ 429 errors |
| **Infrastructure** | Add required header | Request rejected | ❌ 400 errors | ❌ 400 errors | ❌ 400 errors |
| **Infrastructure** | Change wire encoding | Protocol break | ❌ Decode error | ❌ Decode error | ❌ Decode error |
| **Validation** | Stricter input rules | Previously valid rejected | ❌ Rejected | ❌ Rejected | ❌ Rejected |
| **Webhooks** | Change payload structure | Handler parsing fails | ❌ Handler fail | ❌ Handler fail | ❌ Parse error |
| **Webhooks** | Remove webhook field | Handler references missing | ❌ KeyError | ❌ Null | ❌ Compile error |
| **Webhooks** | Change endpoint URL | Hardcoded URLs break | ❌ 404 | ❌ 404 | ❌ 404 |
| **Endpoints** | Remove endpoint | Gone | ❌ 404 | ❌ 404 | ❌ 404 |
| **Endpoints** | Change HTTP method | Method not allowed | ❌ 405 | ❌ 405 | ❌ 405 |
| **Pagination** | Change strategy | Traversal breaks | ❌ Traversal | ❌ Traversal | ❌ Traversal |
| **Dependencies** | Update major dependency | API changes ripple | ❌ API change | ❌ API change | ❌ API change |

**Pattern**: JavaScript/Python break at runtime. Java breaks at runtime or compile time. Rust always breaks at compile time.

---

## MINOR Changes (Additive)

Changes that add capabilities without requiring code changes.

| Category | Change | Implementation | JS/Python | Java | Rust |
|----------|--------|----------------|-----------|------|------|
| **Fields** | Add optional field | `metadata: Option<T>` | ✅ Ignored | ✅ Default null | ✅ Default value |
| **Fields** | Add optional nested | `address.zone: Option<String>` | ✅ Ignored | ✅ Default null | ✅ Default value |
| **Fields** | Deprecate field | Mark `#[deprecated]` | ✅ Works + warn | ✅ Works + warn | ✅ Works + warn |
| **Fields** | Add reserved fields | `reserved 100, 101` | ✅ N/A | ✅ N/A | ✅ N/A |
| **Fields** | Add computed field | `total_amount` getter | ✅ Extra property | ✅ Extra getter | ✅ Extra field |
| **Functions** | Add optional parameter | `idempotency_key = None` | ✅ Works | ✅ Overload | ✅ Default |
| **Functions** | Add new method | `batch_authorize()` | ✅ Unused | ✅ Unused | ✅ Unused |
| **Functions** | Add overload | New param combo | ✅ Works | ✅ Overload | ✅ Works |
| **Functions** | Add return field | Extra response data | ✅ Extra property | ✅ Extra getter | ✅ Extra field |
| **Methods** | Add builder method | `.with_timeout(30)` | ✅ New option | ✅ New option | ✅ New option |
| **Enums** | Add variant | `TRUSTLY` to `Connector` | ✅ New property | ✅ New constant | ⚠️ Need `_ =>` |
| **Enums** | Add event type | `DISPUTE_CREATED` | ✅ New type | ✅ New constant | ⚠️ Need `_ =>` |
| **Enums** | Add status code | `PARTIALLY_REFUNDED` | ✅ New status | ✅ New constant | ⚠️ Need `_ =>` |
| **Enums** | Add currency | `VES` | ✅ New code | ✅ New constant | ✅ Safe |
| **Constructors** | Add constructor overload | `Client::with_config()` | ✅ Alt create | ✅ Alt create | ✅ Alt create |
| **Constructors** | Add builder method | `.with_proxy()` | ✅ New option | ✅ New option | ✅ New option |
| **Config** | Add config option | New `retry_policy` | ✅ Ignored | ✅ Default | ✅ Default |
| **Config** | Add environment preset | `Environment::Staging` | ✅ New target | ✅ New target | ✅ New target |
| **Config** | Add feature flag | `enable_metrics: bool` | ✅ Toggle | ✅ Toggle | ✅ Toggle |
| **Messages** | Add new RPC | `PaymentService.BatchRefund` | ✅ New endpoint | ✅ New endpoint | ✅ New endpoint |
| **Errors** | Add error variant | `RateLimitError` | ✅ Caught by parent | ✅ Caught by parent | ⚠️ Need `_ =>` |
| **Errors** | Add error context | More fields | ✅ More info | ✅ More info | ✅ More info |
| **Infrastructure** | Add compression | brotli (keep gzip) | ✅ Choice | ✅ Choice | ✅ Choice |
| **Infrastructure** | Add region endpoint | `eu.prism.api` | ✅ New region | ✅ New region | ✅ New region |
| **Infrastructure** | Add TLS version | Support TLS 1.3 | ✅ Enhanced | ✅ Enhanced | ✅ Enhanced |
| **Infrastructure** | Add metrics | Prometheus counters | ✅ Observable | ✅ Observable | ✅ Observable |
| **Features** | Add payment method | Samsung Pay | ✅ New variant | ✅ New variant | ⚠️ Need `_ =>` |
| **Features** | Add connector | Peach Payments | ✅ New variant | ✅ New variant | ⚠️ Need `_ =>` |
| **Webhooks** | Add payload field | `payment_method_type` | ✅ Extra data | ✅ Extra data | ✅ Extra data |
| **Webhooks** | Add header | `X-Event-Type` | ✅ Metadata | ✅ Metadata | ✅ Metadata |
| **Pagination** | Add cursor pagination | Alternative to offset | ✅ More options | ✅ More options | ✅ More options |
| **Pagination** | Increase page size | Max 100 → Max 200 | ✅ Flexibility | ✅ Flexibility | ✅ Flexibility |
| **Filtering** | Add filter option | New `currency` filter | ✅ New query | ✅ New query | ✅ New query |
| **Filtering** | Add sort field | Sort by `updated_at` | ✅ New option | ✅ New option | ✅ New option |

**Pattern**: All languages handle additions safely. Rust requires wildcard arm (`_ =>`) for enum additions.

---

## PATCH Changes (Fixes)

Changes that fix bugs or improve performance without changing behavior.

| Category | Change | Example |
|----------|--------|---------|
| **Security** | Fix vulnerability | Webhook signature verification |
| **Security** | Patch auth bypass | OAuth token validation |
| **Security** | Update TLS ciphers | Cipher suite refresh |
| **Bugs** | Fix parsing | Stripe error code mapping |
| **Bugs** | Fix calculation | Tax amount rounding |
| **Bugs** | Fix validation | Accept valid card formats |
| **Bugs** | Fix timeout | Proper deadline handling |
| **Bugs** | Fix memory leak | Connection pool cleanup |
| **Bugs** | Fix race condition | Concurrent request handling |
| **Performance** | Optimize serialization | Faster protobuf encode |
| **Performance** | Reduce latency | Connection reuse |
| **Performance** | Lower memory | Efficient buffering |
| **Reliability** | Fix retry logic | Exponential backoff |
| **Reliability** | Add circuit breaker | Prevent cascade failures |
| **Documentation** | Fix typos | Error message clarity |
| **Cosmetic** | Error message text | `"Card declined"` → `"Payment declined"` |
| **Permissions** | More permissive | Remove OAuth scope requirement |

**Pattern**: All fixes are safe across all languages. No code changes needed.

---



## Rust Core Changes → SDK Impact Reference

Detailed mapping of Rust core changes to cross-language impact.

| Rust Change | Affects | Impact | Mitigation |
|-------------|---------|--------|------------|
| **Proto field removal** | All SDKs | Generated code breaks | Reserve fields, deprecate first |
| **Proto required field** | All SDKs | Validation rejects old requests | Always use optional |
| **Proto field number change** | All SDKs | Wire incompatibility | Never change numbers |
| **Proto field type change** | All SDKs | Deserialization fails | Add new field, deprecate old |
| **Enum variant removal** | Rust, Java | Match/catch incomplete | Deprecate, remove in MAJOR |
| **Enum value change** | All SDKs | Wrong values deserialized | Never change values |
| **FFI function removal** | JS, Python, Java | Runtime symbol error | Version FFI separately |
| **FFI signature change** | JS, Python, Java | Type/argument mismatch | Keep FFI stable |
| **FFI ABI change** | JS, Python, Java | Crashes/corruption | Test across languages |
| **Opaque handle change** | JS, Python, Java | Use-after-free | Version handle types |
| **String encoding change** | JS, Python, Java | Corrupted text | Stick to UTF-8 |
| **Buffer ownership change** | JS, Python, Java | Memory corruption | Clear ownership rules |
| **Error type change** | All SDKs | Error handling fails | Keep error schema stable |
| **Runtime change** | All SDKs | Async tasks fail | Abstract executor |
| **Tokio version change** | All SDKs | Dependency conflicts | Careful version mgmt |
| **Send/Sync addition** | Rust | Threading restriction | Design for thread safety |
| **Drop behavior change** | All SDKs | Resource timing issues | Document Drop guarantees |
| **Panic behavior change** | All SDKs | Abort vs unwind | Set panic policy |
| **Allocator change** | JS, Python, Java | Heap corruption | Use system allocator |
| **Module path change** | Rust | Import failures | Keep paths stable |
| **Public type removal** | Rust | Type errors | Deprecate first |
| **Trait method change** | Rust | Implementations break | Add new trait instead |
| **Feature flag removal** | Rust | Code paths gone | Deprecate gradually |
| **MSRV change** | Rust | Compilation fails | Support old versions |
| **Edition change** | Rust | Syntax requirements | Edition migration guide |

---

## Language Comparison

| Aspect | JS/Python | Java | Rust |
|--------|-----------|------|------|
| **Error timing** | Runtime | Compile or Runtime | Compile time |
| **Adding things** | ✅ Safe | ✅ Safe | ✅ Safe (enums need care) |
| **Removing things** | ❌ Runtime crash | ❌ Runtime/Compile error | ❌ Compile error |
| **Changing things** | ❌ Runtime issues | ❌ Runtime/Compile error | ❌ Compile error |
| **Fixing things** | ✅ Always safe | ✅ Always safe | ✅ Always safe |

---

## Rust Enum Special Case

**Why Rust is different with enums:**

**JavaScript:**
```javascript
// Adding TRUSTLY - old code works fine
if (connector === 'STRIPE') { ... }  // No problem!
```

**Java:**
```java
// Adding TRUSTLY - works with default
switch (connector) {
    case STRIPE: ...; break;
    default: ...;  // TRUSTLY falls here
}
```

**Rust:**
```rust
// ❌ BREAKS when TRUSTLY added
match connector {
    Connector::Stripe => ...,
    Connector::Adyen => ...,
    // Compile error: not exhaustive!
}

// ✅ FIXED with wildcard
match connector {
    Connector::Stripe => ...,
    Connector::Adyen => ...,
    _ => ...,  // Handles TRUSTLY and future variants
}
```

**Solution**: Use `#[non_exhaustive]` attribute on Rust enums to force wildcard patterns.

---

## Why This Approach (Option 2)

### vs Option 1 (Combined Table)

| Aspect | Option 1 (Combined) | Option 2 (Separate) |
|--------|--------------------|--------------------|
| Tables | 1 giant (~200 rows) | 3 focused (~77 rows total) |
| Columns | 7 wide | 5-6 per table |
| Scanning | Hard (mixed versions) | Easy (by severity) |
| Maintenance | Harder | Easier |
| Clarity | Overwhelming | Clear separation |

### Benefits

1. **Clear Separation**: MAJOR, MINOR, PATCH in distinct sections
2. **Language Impact Visible**: See how each language handles breaking/additive changes
3. **Retains Detail**: "Why It Breaks" for MAJOR, "Implementation" for MINOR
4. **Easy to Scan**: Find what you need quickly
5. **Patch Simplicity**: No language columns needed (all safe)

### Row Counts

- **MAJOR**: ~45 rows
- **MINOR**: ~32 rows  
- **PATCH**: ~17 rows
- **Total**: ~94 rows vs ~160 in Option 1

### Recommendation

**Option 2 is the recommended approach** for production use. It balances comprehensiveness with readability and maintainability.
