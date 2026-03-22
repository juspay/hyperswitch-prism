# FFI Tracing Logs Across Language Boundaries

## The Problem

When Rust code using the `tracing` crate is called from Python (or other languages) via FFI, tracing logs do not automatically propagate to the calling language's logging system.

## What Happens

### Rust Side (FFI Library)

```rust
use tracing::{info, error, debug};

#[uniffi::export]
pub fn process_payment(request: PaymentRequest) -> Result<PaymentResponse, String> {
    info!("Processing payment for amount: {}", request.amount);  // ← Where does this go?
    debug!("Request details: {:?}", request);

    // ... processing logic

    error!("Payment failed: insufficient funds");  // ← Where does this go?
}
```

### Python Side (Caller)

```python
import connector_sdk

# Python logging configured
logging.basicConfig(level=logging.INFO)

# Call Rust FFI function
result = connector_sdk.process_payment(request)

# Python logging works fine
logging.info("Payment processed")  # ← Shows in Python logs

# But Rust tracing logs are invisible here!
```

## Current Behavior

### Without Tracing Subscriber Initialization

**Logs disappear into the void.** Rust tracing macros (`info!`, `error!`, etc.) emit events, but if no subscriber is initialized, these events are dropped silently.

```rust
// No subscriber initialized
info!("This message goes nowhere");  // ← Silent, no output
```

### With Tracing Subscriber Initialized (Rust Side)

Logs go to wherever the Rust subscriber is configured to send them:

```rust
// Initialize tracing subscriber in Rust FFI library initialization
use tracing_subscriber;

#[uniffi::export]
pub fn init_sdk() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();
}
```

**Output goes to:**
- Standard output (stdout/stderr) - Python can't intercept this easily
- File on disk - Separate from Python logs
- Structured logging service - Disconnected from Python's logging

**Problem:** Logs are isolated from Python's logging infrastructure.

## Challenges

### 1. Separate Logging Systems

- **Rust**: Uses `tracing` crate with its own subscriber model
- **Python**: Uses `logging` module with handlers and formatters
- **No automatic bridge** between the two

### 2. Context Loss

```python
# Python has request context
request_id = "req_12345"
logger = logging.getLogger(__name__)
logger.info("Processing payment", extra={"request_id": request_id})

# Rust FFI call loses this context
result = connector_sdk.process_payment(request)
# ← Rust logs don't have request_id
```

### 3. Log Interleaving

```
[Python] 2024-01-15 10:00:00 INFO Starting payment processing
[Rust]   2024-01-15 10:00:01 INFO Processing payment  ← Separate log stream
[Python] 2024-01-15 10:00:02 INFO Payment completed
```

Logs from both languages are separate and hard to correlate.

## Solutions

### Solution 1: Capture Rust Logs to String (Current Approach)

Modify FFI functions to capture and return logs alongside results:

```rust
use tracing_subscriber::fmt::MakeWriter;
use std::sync::{Arc, Mutex};

struct StringWriter {
    buffer: Arc<Mutex<String>>,
}

impl std::io::Write for StringWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.push_str(&String::from_utf8_lossy(buf));
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

#[uniffi::export]
pub fn process_payment_with_logs(request: PaymentRequest) -> PaymentResultWithLogs {
    let log_buffer = Arc::new(Mutex::new(String::new()));
    let writer = StringWriter { buffer: log_buffer.clone() };

    // Initialize subscriber that writes to buffer
    let _guard = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .set_default();

    info!("Processing payment");
    let result = process_payment_internal(request);

    let logs = log_buffer.lock().unwrap().clone();

    PaymentResultWithLogs {
        result,
        logs,  // ← Return logs to Python
    }
}
```

**Python side:**
```python
result = connector_sdk.process_payment_with_logs(request)
logging.info(result.logs)  # Forward Rust logs to Python logging
```

**Pros:** Simple, works across all languages
**Cons:** Performance overhead, logs buffered in memory

### Solution 2: Structured Error Returns (Recommended)

Instead of logging, return rich error information in the response:

```rust
#[uniffi::export]
pub fn process_payment(request: PaymentRequest) -> FfiResult {
    match transform_request(request) {
        Ok(http_request) => FfiResult::http_request(http_request),
        Err(e) => {
            // Don't log - return structured error
            FfiResult::integration_error(IntegrationError {
                error_message: e.to_string(),
                error_code: e.error_code(),
                error_category: e.category(),
                suggested_action: e.suggested_action(),
            })
        }
    }
}
```

**Python side:**
```python
result = connector_sdk.process_payment(request)

if result.is_error():
    # Log using Python's logging with full context
    logging.error(
        "Payment failed",
        extra={
            "error_code": result.error.error_code,
            "error_category": result.error.error_category,
            "request_id": request_id,
            "user_id": user_id,
        }
    )
```

**Pros:**
- Clean separation of concerns
- Python controls all logging
- Easy to add Python context (request_id, user_id, etc.)
- No performance overhead

**Cons:**
- Internal Rust debugging harder (no intermediate logs)
- Requires discipline to return rich errors

### Solution 3: Callback-Based Logging

Pass a logging callback from Python to Rust:

```rust
#[uniffi::export]
pub fn process_payment(
    request: PaymentRequest,
    log_callback: Box<dyn LogCallback>,
) -> Result<PaymentResponse, String> {
    log_callback.log("INFO", "Processing payment");

    // ... processing

    if error {
        log_callback.log("ERROR", "Payment failed");
    }
}
```

**Python side:**
```python
class PythonLogger:
    def log(self, level: str, message: str):
        logging.log(getattr(logging, level), message)

result = connector_sdk.process_payment(request, PythonLogger())
```

**Pros:** Rust logs appear in Python's logging system
**Cons:** Complex, callback overhead on every log, harder to maintain

### Solution 4: Separate Log Aggregation

Keep logs separate but aggregate them externally:

```
Rust FFI → stdout → Docker logs → Log aggregator (e.g., Loki, Datadog)
                                          ↑
Python   → stdout → Docker logs ──────────┘
```

**Pros:**
- No code changes needed
- Works at infrastructure level
- Both log streams preserved

**Cons:**
- Requires infrastructure setup
- Correlation still challenging

## Recommendation for This Project

**Use Solution 2: Structured Error Returns**

Why:
1. **Clean separation** - Python owns logging, Rust focuses on transformation
2. **Better observability** - Python can add request context, user context, etc.
3. **Performance** - No logging overhead in hot path
4. **Simplicity** - No complex callback mechanisms or log capture

**For debugging Rust code:**
- Use tests with tracing subscriber initialized
- Use debug builds with tracing to stderr
- Add `#[cfg(test)]` tracing initialization for development

**For production:**
- Return rich errors via `IntegrationError` and `ConnectorResponseTransformationError`
- Let Python/JavaScript/Java SDKs handle all logging with their native logging infrastructure
- Include error codes, categories, and suggested actions for observability

## Example: Current vs Recommended

### Current (Problematic)

```rust
#[uniffi::export]
pub fn transform_request(req: PaymentRequest) -> Result<HttpRequest, String> {
    tracing::info!("Transforming request");  // ← Lost in FFI

    if req.amount.is_none() {
        tracing::error!("Missing amount");  // ← Lost in FFI
        return Err("Missing amount".to_string());
    }

    // ...
}
```

### Recommended

```rust
#[uniffi::export]
pub fn transform_request(req: PaymentRequest) -> FfiResult {
    // No tracing - return structured errors

    if req.amount.is_none() {
        return FfiResult::integration_error(IntegrationError {
            error_message: "Missing required field 'amount'".to_string(),
            error_code: "MISSING_REQUIRED_FIELD".to_string(),
            error_category: IntegrationErrorCategory::ValidationError,
            suggested_action: Some("Provide the 'amount' field in your request".to_string()),
        });
    }

    // ...
}
```

**Python logs everything:**
```python
result = sdk.transform_request(request)

if isinstance(result, IntegrationError):
    logger.error(
        "Request transformation failed",
        extra={
            "error_code": result.error_code,
            "error_category": result.error_category,
            "request_id": ctx.request_id,
            "merchant_id": ctx.merchant_id,
        }
    )
```

## Summary

Tracing logs across FFI boundaries are lost by default because Rust and Python have separate logging ecosystems. The recommended approach is to avoid logging in Rust FFI code entirely and instead return structured error information that Python can log with full application context.
