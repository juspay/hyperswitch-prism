# Versioning Policy - Option 1: Combined Single Table

> **Note**: This is the "Combined Table" approach. It's comprehensive but unwieldy with ~200 rows. Consider using Option 2 (Enhanced Separate Tables) for better readability.

**Principle**: Same `MAJOR.MINOR` = same protocol compatibility across all languages.

| Version | Rule |
|---------|------|
| **MAJOR** | Breaking change - requires code modification |
| **MINOR** | New capability - additive only, no changes needed |
| **PATCH** | Fix/security - safe to auto-update |

---

## Combined Version Reference Table

| Category | Change | Version | JS/Python | Java | Rust | Details |
|----------|--------|---------|-----------|------|------|---------|
| **Fields** | Remove field | **MAJOR** | ❌ undefined | ❌ NPE | ❌ Compile error | Code referencing field breaks |
| **Fields** | Make optional required | **MAJOR** | ❌ Validation error | ❌ Validation error | ❌ Compile error | Existing requests rejected |
| **Fields** | Change field type | **MAJOR** | ❌ Type coercion fails | ❌ ClassCastException | ❌ Type mismatch | Serialization fails |
| **Fields** | Change format | **MAJOR** | ❌ Parse error | ❌ Parse error | ❌ Parse error | Validation fails |
| **Fields** | Change timestamp format | **MAJOR** | ❌ Date parsing fails | ❌ Date parsing fails | ❌ Parse error | Parsing logic fails |
| **Fields** | Change ID format | **MAJOR** | ❌ Validation fails | ❌ Validation fails | ❌ Validation fails | Length/prefix check fails |
| **Fields** | Change field semantics | **MAJOR** | ❌ Logic error | ❌ Logic error | ❌ Logic error | Same name, different meaning |
| **Fields** | Required nested field | **MAJOR** | ❌ Validation error | ❌ Validation error | ❌ Compile error | Objects missing field rejected |
| **Fields** | Add optional field | **MINOR** | ✅ Ignored | ✅ Default null | ✅ Default value | Omitted uses default |
| **Fields** | Add optional nested | **MINOR** | ✅ Ignored | ✅ Default null | ✅ Default value | Safe nesting |
| **Fields** | Deprecate field | **MINOR** | ✅ Works + warning | ✅ Works + warning | ✅ Works + warning | Still functional |
| **Fields** | Add reserved fields | **MINOR** | ✅ N/A | ✅ N/A | ✅ N/A | Prevents future reuse |
| **Fields** | Add computed field | **MINOR** | ✅ Extra property | ✅ Extra getter | ✅ Extra field | Derived from existing |
| **Fields** | Add field alias | **MINOR** | ✅ Both work | ✅ Both work | ✅ Both work | Old and new names work |
| **Fields** | Add metadata/map | **MINOR** | ✅ Flexible | ✅ Flexible | ✅ Flexible | Key-value storage |
| **Fields** | Fix validation bug | **PATCH** | ✅ Correct | ✅ Correct | ✅ Correct | Now accepts valid input |
| **Fields** | Fix parse error | **PATCH** | ✅ Correct | ✅ Correct | ✅ Correct | Parsing fixed |
| **Functions** | Add required parameter | **MAJOR** | ❌ TypeError | ❌ Compile error | ❌ Compile error | Missing argument |
| **Functions** | Remove parameter | **MAJOR** | ❌ Too many args | ❌ Compile error | ❌ Compile error | Calls passing param fail |
| **Functions** | Reorder parameters | **MAJOR** | ❌ Silent wrong args | ❌ Compile error | ❌ Compile error | Wrong binding |
| **Functions** | Change parameter type | **MAJOR** | ❌ Type coercion fails | ❌ Compile error | ❌ Type mismatch | Argument rejected |
| **Functions** | Rename parameter (named) | **MAJOR** | ❌ Keyword error | ❌ Named arg error | ✅ Works | Named lookup fails |
| **Functions** | Change return type | **MAJOR** | ❌ Property access fails | ❌ Cast fails | ❌ Type mismatch | Destructuring breaks |
| **Functions** | Remove return field | **MAJOR** | ❌ Undefined | ❌ Null pointer | ❌ Compile error | Field access fails |
| **Functions** | Change method name | **MAJOR** | ❌ AttributeError | ❌ Compile error | ❌ Compile error | Method not found |
| **Functions** | Remove method | **MAJOR** | ❌ AttributeError | ❌ Compile error | ❌ Compile error | Method not found |
| **Functions** | Change visibility | **MAJOR** | ❌ Access denied | ❌ Access denied | ❌ Compile error | Private access |
| **Functions** | Static/instance swap | **MAJOR** | ❌ Invocation fails | ❌ Compile error | ❌ Compile error | Wrong calling convention |
| **Functions** | Add optional parameter | **MINOR** | ✅ Works | ✅ Overload | ✅ Default | Old calls work |
| **Functions** | Add new method | **MINOR** | ✅ Unused | ✅ Unused | ✅ Unused | Old code ignores it |
| **Functions** | Add overload | **MINOR** | ✅ Works | ✅ Overload | ✅ Works | Java/Kotlin pattern |
| **Functions** | Add return field | **MINOR** | ✅ Extra property | ✅ Extra getter | ✅ Extra field | More data |
| **Functions** | Add default impl | **MINOR** | ✅ Back compat | ✅ Back compat | ✅ Back compat | Trait method |
| **Functions** | Fix return value | **PATCH** | ✅ Correct | ✅ Correct | ✅ Correct | Bug fix |
| **Functions** | Fix parameter handling | **PATCH** | ✅ Correct | ✅ Correct | ✅ Correct | Bug fix |
| **Enums** | Remove variant | **MAJOR** | ❌ Undefined | ❌ IllegalArgument | ❌ Compile error | Match arm missing |
| **Enums** | Change value | **MAJOR** | ❌ Wrong logic | ❌ Wrong logic | ❌ Wrong logic | Numeric mapping wrong |
| **Enums** | Change semantics | **MAJOR** | ❌ State machine breaks | ❌ State machine breaks | ❌ Logic error | Same name, different meaning |
| **Enums** | Reorder | **MAJOR** | ❌ Ordering wrong | ❌ Ordering wrong | ✅ Safe | Assumptions broken |
| **Enums** | Add variant | **MINOR** | ✅ New property | ✅ New constant | ⚠️ Need `_ =>` | Safe except Rust enums |
| **Enums** | Add event type | **MINOR** | ✅ New type | ✅ New constant | ⚠️ Need `_ =>` | Webhook event |
| **Enums** | Add status code | **MINOR** | ✅ New status | ✅ New constant | ⚠️ Need `_ =>` | Payment status |
| **Enums** | Add currency | **MINOR** | ✅ New code | ✅ New constant | ✅ Safe addition | Geographic expansion |
| **Enums** | Add placeholder | **MINOR** | ✅ Reserved | ✅ Reserved | ✅ Safe | Future expansion |
| **Constructors** | Remove no-arg constructor | **MAJOR** | ❌ Instantiation fails | ❌ Compile error | ❌ Compile error | Default construction breaks |
| **Constructors** | Make param required | **MAJOR** | ❌ Instantiation fails | ❌ Compile error | ❌ Compile error | Missing argument |
| **Constructors** | Remove builder method | **MAJOR** | ❌ Chain fails | ❌ Compile error | ❌ Compile error | Fluent API breaks |
| **Constructors** | Add param | **MINOR** | ✅ Default/kwargs | ✅ Overload | ✅ Default | Old calls work |
| **Constructors** | Add builder method | **MINOR** | ✅ New option | ✅ New option | ✅ New option | Fluent extension |
| **Constructors** | Add factory | **MINOR** | ✅ Convenience | ✅ Convenience | ✅ Convenience | Alternative creation |
| **Constructors** | Add copy method | **MINOR** | ✅ Immutable | ✅ Immutable | ✅ Immutable | With-methods |
| **Config** | Remove config option | **MAJOR** | ❌ KeyError | ❌ Null/Error | ❌ Compile error | Setting missing |
| **Config** | Change default value | **MAJOR** | ❌ Behavior change | ❌ Behavior change | ❌ Behavior change | Different outcome |
| **Config** | Change format | **MAJOR** | ❌ Parse error | ❌ Parse error | ❌ Parse error | Parser fails |
| **Config** | Add config option | **MINOR** | ✅ Ignored | ✅ Default | ✅ Default | Uses default |
| **Config** | Add preset | **MINOR** | ✅ New target | ✅ New target | ✅ New target | Environment |
| **Config** | Add feature flag | **MINOR** | ✅ Toggle | ✅ Toggle | ✅ Toggle | Optional feature |
| **Behavior** | Change default action | **MAJOR** | ❌ Different outcome | ❌ Different outcome | ❌ Different outcome | Same code, different result |
| **Behavior** | Change pagination | **MAJOR** | ❌ Data assumptions | ❌ Data assumptions | ❌ Data assumptions | Page size changed |
| **Behavior** | Change sort order | **MAJOR** | ❌ Wrong order | ❌ Wrong order | ❌ Wrong order | Ordering changed |
| **Behavior** | Change null semantics | **MAJOR** | ❌ Logic error | ❌ Logic error | ❌ Logic error | Null means different |
| **Behavior** | Change boolean default | **MAJOR** | ❌ Security change | ❌ Security change | ❌ Security change | Different behavior |
| **Behavior** | Change callback timing | **MAJOR** | ❌ Race conditions | ❌ Race conditions | ❌ Race conditions | Sync/async swap |
| **Behavior** | Fix calculation | **PATCH** | ✅ Correct | ✅ Correct | ✅ Correct | Math fix |
| **Behavior** | Fix logic bug | **PATCH** | ✅ Correct | ✅ Correct | ✅ Correct | Logic fix |
| **Errors** | Remove error variant | **MAJOR** | ❌ Except clause fails | ❌ Catch fails | ❌ Match incomplete | Catch/match fails |
| **Errors** | Change error code | **MAJOR** | ❌ String match fails | ❌ String match fails | ❌ Match fails | Matching fails |
| **Errors** | Change hierarchy | **MAJOR** | ❌ isinstance fails | ❌ instanceof fails | ❌ Downcast fails | Type checking fails |
| **Errors** | Add error variant | **MINOR** | ✅ Caught by parent | ✅ Caught by parent | ⚠️ Need `_ =>` | Safe except Rust |
| **Infrastructure** | Drop platform support | **MAJOR** | ❌ Import error | ❌ ClassNotFound | ❌ Link error | Build fails |
| **Infrastructure** | Change rate limits | **MAJOR** | ❌ 429 errors | ❌ 429 errors | ❌ 429 errors | Throttled |
| **Infrastructure** | Add required header | **MAJOR** | ❌ 400 errors | ❌ 400 errors | ❌ 400 errors | Request rejected |
| **Infrastructure** | Change wire encoding | **MAJOR** | ❌ Decode error | ❌ Decode error | ❌ Decode error | Protocol break |
| **Infrastructure** | Remove compression | **MAJOR** | ❌ Cannot decode | ❌ Cannot decode | ❌ Cannot decode | Client fails |
| **Infrastructure** | Add auto-retry | **MAJOR** | ❌ Idempotency issues | ❌ Idempotency issues | ❌ Idempotency issues | Double execution |
| **Infrastructure** | Change retry behavior | **MAJOR** | ❌ Unexpected retries | ❌ Unexpected retries | ❌ Unexpected retries | Logic change |
| **Infrastructure** | Change TLS requirements | **MAJOR** | ❌ Handshake fails | ❌ Handshake fails | ❌ Handshake fails | Connection fails |
| **Infrastructure** | Change cert validation | **MAJOR** | ❌ Cert rejected | ❌ Cert rejected | ❌ Cert rejected | Trust issues |
| **Infrastructure** | Add compression | **MINOR** | ✅ brotli option | ✅ brotli option | ✅ brotli option | Alongside gzip |
| **Infrastructure** | Add region endpoint | **MINOR** | ✅ eu.prism.api | ✅ eu.prism.api | ✅ eu.prism.api | Geographic expansion |
| **Infrastructure** | Add TLS version | **MINOR** | ✅ TLS 1.3 | ✅ TLS 1.3 | ✅ TLS 1.3 | Enhanced security |
| **Infrastructure** | Add metrics | **MINOR** | ✅ Prometheus | ✅ Prometheus | ✅ Prometheus | Observability |
| **Infrastructure** | Add tracing | **MINOR** | ✅ OpenTelemetry | ✅ OpenTelemetry | ✅ OpenTelemetry | Debugging |
| **Infrastructure** | Security fix | **PATCH** | ✅ Safe | ✅ Safe | ✅ Safe | Vulnerability patched |
| **Infrastructure** | Performance fix | **PATCH** | ✅ Faster | ✅ Faster | ✅ Faster | Optimization |
| **Validation** | Stricter rules | **MAJOR** | ❌ Input rejected | ❌ Input rejected | ❌ Input rejected | Previously valid now invalid |
| **Validation** | New auth scope | **MAJOR** | ❌ Token fails | ❌ Token fails | ❌ Token fails | OAuth insufficient |
| **Validation** | IP whitelist | **MAJOR** | ❌ Blocked | ❌ Blocked | ❌ Blocked | Firewall rule |
| **Validation** | Reduce max size | **MAJOR** | ❌ Rejected | ❌ Rejected | ❌ Rejected | Payload too large |
| **Validation** | Add soft validation | **MINOR** | ✅ Warning | ✅ Warning | ✅ Warning | Non-blocking |
| **Validation** | Accept more formats | **MINOR** | ✅ Broader | ✅ Broader | ✅ Broader | More permissive |
| **Validation** | Relax validation | **MINOR** | ✅ More lenient | ✅ More lenient | ✅ More lenient | Accepts more |
| **Validation** | Fix validation bug | **PATCH** | ✅ Correct | ✅ Correct | ✅ Correct | Now accepts valid |
| **Webhooks** | Change payload structure | **MAJOR** | ❌ Handler fails | ❌ Handler fails | ❌ Parse error | JSON shape different |
| **Webhooks** | Remove field | **MAJOR** | ❌ KeyError | ❌ Null pointer | ❌ Compile error | Handler references missing |
| **Webhooks** | Change auth method | **MAJOR** | ❌ Verify fails | ❌ Verify fails | ❌ Verify fails | Signature check fails |
| **Webhooks** | Reduce retries | **MAJOR** | ❌ Delivery fails | ❌ Delivery fails | ❌ Delivery fails | 3→1 retry |
| **Webhooks** | Change endpoint URL | **MAJOR** | ❌ 404 error | ❌ 404 error | ❌ 404 error | Hardcoded URL |
| **Webhooks** | Remove endpoint | **MAJOR** | ❌ 404 error | ❌ 404 error | ❌ 404 error | Gone |
| **Webhooks** | Change HTTP method | **MAJOR** | ❌ 405 error | ❌ 405 error | ❌ 405 error | Method not allowed |
| **Webhooks** | Add payload field | **MINOR** | ✅ Extra data | ✅ Extra data | ✅ Extra data | More context |
| **Webhooks** | Add header | **MINOR** | ✅ Metadata | ✅ Metadata | ✅ Metadata | Extra info |
| **Webhooks** | Add signature version | **MINOR** | ✅ Better auth | ✅ Better auth | ✅ Better auth | Enhanced security |
| **Webhooks** | Add filter options | **MINOR** | ✅ Subset | ✅ Subset | ✅ Subset | Selective delivery |
| **Webhooks** | Add retry visibility | **MINOR** | ✅ Debug header | ✅ Debug header | ✅ Debug header | Retry count |
| **Webhooks** | Fix delivery bug | **PATCH** | ✅ Reliable | ✅ Reliable | ✅ Reliable | Delivery fixed |
| **Pagination** | Change strategy | **MAJOR** | ❌ Traversal breaks | ❌ Traversal breaks | ❌ Traversal breaks | Offset→cursor |
| **Pagination** | Reduce max size | **MAJOR** | ❌ Rejected | ❌ Rejected | ❌ Rejected | Max 100→50 |
| **Pagination** | Remove parameter | **MAJOR** | ❌ Unknown param | ❌ Unknown param | ❌ Compile error | Query fails |
| **Pagination** | Change cursor format | **MAJOR** | ❌ Cursor invalid | ❌ Cursor invalid | ❌ Parse error | Stored cursors break |
| **Pagination** | Add cursor pagination | **MINOR** | ✅ Alternative | ✅ Alternative | ✅ Alternative | To offset |
| **Pagination** | Increase max size | **MINOR** | ✅ More results | ✅ More results | ✅ More results | Flexibility |
| **Pagination** | Add metadata | **MINOR** | ✅ Total count | ✅ Total count | ✅ Total count | Extra info |
| **Pagination** | Add links | **MINOR** | ✅ HATEOAS | ✅ HATEOAS | ✅ HATEOAS | Navigation |
| **Pagination** | Fix count bug | **PATCH** | ✅ Correct | ✅ Correct | ✅ Correct | Count fixed |
| **Filtering** | Remove filter option | **MAJOR** | ❌ Ignored/rejected | ❌ Ignored/rejected | ❌ Compile error | Filter gone |
| **Filtering** | Change operator | **MAJOR** | ❌ Not recognized | ❌ Not recognized | ❌ Not recognized | `eq`→`equals` |
| **Filtering** | Change logic | **MAJOR** | ❌ Wrong results | ❌ Wrong results | ❌ Wrong results | AND→OR |
| **Filtering** | Remove sort field | **MAJOR** | ❌ Sort rejected | ❌ Sort rejected | ❌ Compile error | Can't sort |
| **Filtering** | Change default sort | **MAJOR** | ❌ Wrong order | ❌ Wrong order | ❌ Wrong order | Order changed |
| **Filtering** | Add filter option | **MINOR** | ✅ New query | ✅ New query | ✅ New query | More power |
| **Filtering** | Add operator | **MINOR** | ✅ More flexible | ✅ More flexible | ✅ More flexible | `startsWith` |
| **Filtering** | Add sort field | **MINOR** | ✅ New option | ✅ New option | ✅ New option | Sort by new field |
| **Filtering** | Add sort direction | **MINOR** | ✅ More control | ✅ More control | ✅ More control | Explicit asc/desc |
| **Batch/Streaming** | Reduce max batch size | **MAJOR** | ❌ Rejected | ❌ Rejected | ❌ Rejected | Too large |
| **Batch/Streaming** | Change batch format | **MAJOR** | ❌ Parser fails | ❌ Parser fails | ❌ Parse error | Structure change |
| **Batch/Streaming** | Change response format | **MAJOR** | ❌ Parser fails | ❌ Parser fails | ❌ Parse error | Response different |
| **Batch/Streaming** | Change chunk size | **MAJOR** | ❌ Buffer issues | ❌ Buffer issues | ❌ Buffer issues | Memory |
| **Batch/Streaming** | Change format | **MAJOR** | ❌ Parser fails | ❌ Parser fails | ❌ Parse error | Protocol |
| **Batch/Streaming** | Remove streaming | **MAJOR** | ❌ Memory issues | ❌ Memory issues | ❌ Memory issues | Full response |
| **Batch/Streaming** | Add batch support | **MINOR** | ✅ New feature | ✅ New feature | ✅ New feature | Efficiency |
| **Batch/Streaming** | Add streaming | **MINOR** | ✅ Memory efficient | ✅ Memory efficient | ✅ Memory efficient | Large payloads |
| **Caching/Observability** | Change cache keys | **MAJOR** | ❌ Stale data | ❌ Stale data | ❌ Stale data | Invalidation wrong |
| **Caching/Observability** | Reduce TTL | **MAJOR** | ❌ Perf degrade | ❌ Perf degrade | ❌ Perf degrade | More requests |
| **Caching/Observability** | Remove caching | **MAJOR** | ❌ Rate limited | ❌ Rate limited | ❌ Rate limited | All to origin |
| **Caching/Observability** | Change log levels | **MAJOR** | ❌ Log spam | ❌ Log spam | ❌ Log spam | Volume change |
| **Caching/Observability** | Change log schema | **MAJOR** | ❌ Parsers break | ❌ Parsers break | ❌ Parsers break | JSON shape |
| **Caching/Observability** | Remove correlation ID | **MAJOR** | ❌ Tracing broken | ❌ Tracing broken | ❌ Tracing broken | Tracking lost |
| **Caching/Observability** | Change metric names | **MAJOR** | ❌ Dashboards break | ❌ Dashboards break | ❌ Dashboards break | Alerts fail |
| **Caching/Observability** | Add metrics | **MINOR** | ✅ More data | ✅ More data | ✅ More data | Observability |
| **Caching/Observability** | Add tracing | **MINOR** | ✅ Debuggable | ✅ Debuggable | ✅ Debuggable | Distributed |
| **Caching/Observability** | Fix log bug | **PATCH** | ✅ Correct | ✅ Correct | ✅ Correct | Log fix |
| **Memory/Threading** | Change struct layout | **MAJOR** | ❌ FFI crash | ❌ JNI crash | ❌ ABI break | Memory corruption |
| **Memory/Threading** | Change alignment | **MAJOR** | ❌ Corruption | ❌ Corruption | ❌ UB | Alignment wrong |
| **Memory/Threading** | Increase footprint | **MAJOR** | ❌ OOM errors | ❌ OOM errors | ❌ OOM errors | Memory pressure |
| **Memory/Threading** | Make !Send/!Sync | **MAJOR** | ❌ Thread errors | ❌ Thread errors | ❌ Compile error | Can't share |
| **Memory/Threading** | Change thread safety | **MAJOR** | ❌ Race conditions | ❌ Race conditions | ❌ Data races | Interior mutability |
| **Memory/Threading** | Remove thread pool | **MAJOR** | ❌ Deadlocks | ❌ Deadlocks | ❌ Deadlocks | Sequential only |
| **Memory/Threading** | Optimize layout | **PATCH** | ✅ Faster | ✅ Faster | ✅ Faster | Transparent |
| **Memory/Threading** | Fix race condition | **PATCH** | ✅ Correct | ✅ Correct | ✅ Correct | Safe now |
| **Dependencies** | Update major dep | **MAJOR** | ❌ API changes | ❌ API changes | ❌ API changes | Ripple effect |
| **Dependencies** | Remove re-export | **MAJOR** | ❌ Import fails | ❌ Import fails | ❌ Import fails | Not found |
| **Dependencies** | Change MSRV | **MAJOR** | N/A | N/A | ❌ Compile fails | Old rustc |
| **Dependencies** | Change edition | **MAJOR** | N/A | N/A | ❌ Syntax errors | Edition compat |
| **Dependencies** | Require nightly | **MAJOR** | N/A | N/A | ❌ Stable fails | Nightly only |
| **Dependencies** | Pin dependency | **MAJOR** | ❌ Conflicts | ❌ Conflicts | ❌ Conflicts | Resolution |
| **Dependencies** | Update minor | **PATCH** | ✅ Bug fixes | ✅ Bug fixes | ✅ Bug fixes | Transparent |
| **Dependencies** | Update patch | **PATCH** | ✅ Security | ✅ Security | ✅ Security | Transparent |

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

## Analysis

### Pros
- One place to look for any change
- See version and language impact together
- Comprehensive reference

### Cons
- Extremely wide table (hard to read on small screens)
- ~200 rows - difficult to scan
- MAJOR/MINOR/PATCH mixed together
- Information overload

### Row Count
- ~160 rows total
- 7 columns wide

### Recommendation
This approach is comprehensive but unwieldy. Consider using **Option 2 (Enhanced Separate Tables)** for better readability and maintenance.
