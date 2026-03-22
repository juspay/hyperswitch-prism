# What Rust Logs Are Visible to SDK Users?

## The Critical Question

When a Python/JavaScript/Java developer uses the SDK and calls Rust FFI code, what Rust logging output appears in their terminal or logs?

## Short Answer

| Rust Logging Method | Visible to SDK Users? | Notes |
|---------------------|----------------------|-------|
| `tracing::info!()` etc | **NO** (by default) | Requires subscriber initialization |
| `println!()` | **YES** | Always prints to stdout |
| `eprintln!()` | **YES** | Always prints to stderr |
| `dbg!()` | **YES** | Always prints to stderr |
| `log::info!()` etc | **NO** (by default) | Requires logger initialization |
| `panic!()` | **YES** (crashes) | Terminates program |

## Detailed Breakdown

### 1. tracing::info!, tracing::error!, etc.

```rust
use tracing::{info, error, debug};

#[uniffi::export]
pub fn process_payment(request: PaymentRequest) -> FfiResult {
    info!("Processing payment");           // ← NOT visible to SDK users
    debug!("Request: {:?}", request);      // ← NOT visible to SDK users
    error!("Payment failed");              // ← NOT visible to SDK users

    // ...
}
```

**Behavior:**
- **Without subscriber initialized:** Completely silent, no output anywhere
- **With subscriber initialized:** Outputs to whatever subscriber is configured for

**Default in FFI libraries:** No subscriber initialized → **SILENT** ✅

**SDK user sees:**
```python
result = sdk.process_payment(request)  # No Rust output appears
```

**Why this is safe:**
- Rust FFI library doesn't initialize a tracing subscriber by default
- tracing events are dropped silently
- SDK users see nothing

**Can be enabled for debugging:**
```rust
#[cfg(test)]
mod tests {
    use tracing_subscriber;

    #[test]
    fn test_payment() {
        // Initialize subscriber for debugging tests
        tracing_subscriber::fmt::init();

        // Now tracing logs will show in test output
        process_payment(request);
    }
}
```

---

### 2. println!()

```rust
#[uniffi::export]
pub fn process_payment(request: PaymentRequest) -> FfiResult {
    println!("Processing payment for ${}", request.amount);  // ← VISIBLE to SDK users!

    // ...
}
```

**Behavior:**
- **Always prints to stdout**
- No initialization needed
- Cannot be disabled

**SDK user sees:**
```python
result = sdk.process_payment(request)
# Terminal shows: Processing payment for $100
```

**This is a PROBLEM for production SDK:**
```python
# Developer using your SDK in production
for payment in payments:
    result = sdk.process_payment(payment)  # ← Rust debugging output pollutes their logs!

# Output:
# Processing payment for $100
# Processing payment for $50
# Processing payment for $200
# ... (thousands of lines)
```

**Rule: NEVER use println!() in FFI code** ❌

---

### 3. eprintln!()

```rust
#[uniffi::export]
pub fn process_payment(request: PaymentRequest) -> FfiResult {
    eprintln!("ERROR: Payment failed");  // ← VISIBLE to SDK users!

    // ...
}
```

**Behavior:**
- **Always prints to stderr**
- No initialization needed
- Cannot be disabled

**SDK user sees:**
```python
result = sdk.process_payment(request)
# Terminal shows: ERROR: Payment failed
```

**Common misuse:**
```rust
// BAD: Developer adds eprintln! for debugging
eprintln!("DEBUG: Got here");
eprintln!("DEBUG: request = {:?}", request);

// These ALL appear in SDK user's terminal/logs!
```

**Rule: NEVER use eprintln!() in FFI code** ❌

---

### 4. dbg!()

```rust
#[uniffi::export]
pub fn process_payment(request: PaymentRequest) -> FfiResult {
    dbg!(request.amount);  // ← VISIBLE to SDK users!

    // ...
}
```

**Behavior:**
- **Always prints to stderr** with file location and variable name
- Cannot be disabled (in release builds, requires explicit compilation flag)

**SDK user sees:**
```python
result = sdk.process_payment(request)
# Terminal shows: [src/payment.rs:42] request.amount = 100
```

**Rule: NEVER commit dbg!() in FFI code** ❌

---

### 5. log::info!() (log crate)

```rust
use log::{info, error};

#[uniffi::export]
pub fn process_payment(request: PaymentRequest) -> FfiResult {
    info!("Processing payment");  // ← NOT visible (by default)

    // ...
}
```

**Behavior:**
- Similar to tracing - requires logger initialization
- Without logger: Silent
- With logger: Outputs to configured destination

**Default in FFI libraries:** No logger initialized → **SILENT** ✅

---

### 6. panic!()

```rust
#[uniffi::export]
pub fn process_payment(request: PaymentRequest) -> FfiResult {
    if request.amount.is_none() {
        panic!("Amount is required!");  // ← CRASHES SDK user's application!
    }

    // ...
}
```

**Behavior:**
- **Terminates the program** (or unwinds the stack in FFI context)
- Panic message printed to stderr
- SDK user's application crashes

**SDK user sees:**
```python
result = sdk.process_payment(request)
# Terminal shows: thread 'main' panicked at 'Amount is required!', src/payment.rs:42
# Python process crashes or shows FFI error
```

**Rule: NEVER panic!() in FFI code - return errors instead** ❌

---

## What Should You Use in FFI Code?

### ✅ SAFE: tracing (with no subscriber)

```rust
use tracing::{info, error, debug, instrument};

#[uniffi::export]
#[instrument(skip(request))]
pub fn process_payment(request: PaymentRequest) -> FfiResult {
    info!("Processing payment");  // Silent by default
    debug!("Request details: {:?}", request);  // Silent by default

    // Can be enabled in tests:
    // cargo test -- --nocapture

    // ...
}
```

**Why safe:**
- Silent by default (no subscriber)
- Can be enabled for testing/debugging
- Structured logging when needed
- No performance overhead when disabled

### ✅ SAFE: Structured error returns

```rust
#[uniffi::export]
pub fn process_payment(request: PaymentRequest) -> FfiResult {
    if request.amount.is_none() {
        return FfiResult::integration_error(IntegrationError {
            error_message: "Missing required field 'amount'".to_string(),
            error_code: "MISSING_REQUIRED_FIELD".to_string(),
            error_category: IntegrationErrorCategory::ValidationError,
            suggested_action: Some("Provide the 'amount' field".to_string()),
        });
    }

    // ...
}
```

**SDK user controls logging:**
```python
result = sdk.process_payment(request)

if isinstance(result, IntegrationError):
    # Python developer decides how to log
    logging.error(f"Payment failed: {result.error_message}")
```

### ❌ UNSAFE: println!, eprintln!, dbg!

```rust
// NEVER DO THIS in FFI code:
println!("Debug info");        // ← Pollutes SDK user's output
eprintln!("Error occurred");   // ← Pollutes SDK user's output
dbg!(some_variable);           // ← Pollutes SDK user's output
```

---

## How to Debug FFI Code Without Polluting SDK Output

### Option 1: Use tracing in tests only

```rust
#[cfg(test)]
mod tests {
    use tracing_subscriber;

    #[test]
    fn test_payment_processing() {
        // Initialize tracing for this test
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .try_init();

        // Now you'll see tracing output
        let result = process_payment(request);

        assert!(result.is_ok());
    }
}
```

### Option 2: Conditional compilation

```rust
#[uniffi::export]
pub fn process_payment(request: PaymentRequest) -> FfiResult {
    #[cfg(debug_assertions)]
    eprintln!("DEBUG: Processing payment");  // Only in debug builds

    // Production code uses tracing (silent)
    tracing::info!("Processing payment");

    // ...
}
```

**Warning:** Still visible in debug builds, but at least not in release

### Option 3: Feature flag

```toml
# Cargo.toml
[features]
debug-logging = []
```

```rust
#[uniffi::export]
pub fn process_payment(request: PaymentRequest) -> FfiResult {
    #[cfg(feature = "debug-logging")]
    eprintln!("DEBUG: Processing payment");

    // ...
}
```

**Build with logging:**
```bash
cargo build --features debug-logging
```

---

## Real-World Example

### BAD: Leaks debugging output to SDK users

```rust
#[uniffi::export]
pub fn transform_request(request: PaymentRequest) -> FfiResult {
    println!("Starting transformation");  // ← SDK user sees this

    if request.card_number.is_none() {
        eprintln!("ERROR: Missing card number");  // ← SDK user sees this
        return FfiResult::integration_error(...);
    }

    println!("Transformation complete");  // ← SDK user sees this
    FfiResult::http_request(...)
}
```

**SDK user experience:**
```python
# Their Python code
for payment in payments:
    result = sdk.transform_request(payment)

# Their terminal output:
Starting transformation
ERROR: Missing card number
Starting transformation
Transformation complete
Starting transformation
Transformation complete
# ... etc (thousands of lines they didn't ask for)
```

### GOOD: Silent with structured errors

```rust
#[uniffi::export]
pub fn transform_request(request: PaymentRequest) -> FfiResult {
    // Use tracing (silent by default)
    tracing::info!("Starting transformation");

    if request.card_number.is_none() {
        tracing::error!("Missing card number");

        // Return structured error
        return FfiResult::integration_error(IntegrationError {
            error_message: "Missing required field 'card_number'".to_string(),
            error_code: "MISSING_REQUIRED_FIELD".to_string(),
            error_category: IntegrationErrorCategory::ValidationError,
            suggested_action: Some("Provide card number".to_string()),
        });
    }

    tracing::info!("Transformation complete");
    FfiResult::http_request(...)
}
```

**SDK user experience:**
```python
# Their Python code
for payment in payments:
    result = sdk.transform_request(payment)

    if isinstance(result, IntegrationError):
        # THEY control logging in Python
        logger.error(f"Transformation failed: {result.error_code}")

# Their terminal: clean, only their own logs
```

---

## Summary

**What SDK users see from Rust code:**

✅ **Nothing** from `tracing::info!()` etc. (safe to use)

❌ **Everything** from `println!()`, `eprintln!()`, `dbg!()` (never use in FFI)

**Best practice for FFI code:**

1. Use `tracing` for internal debugging (silent by default)
2. Return structured errors instead of logging errors
3. Let SDK users control all logging in their language
4. Never use `println!`, `eprintln!`, `dbg!` in production FFI code
5. Initialize tracing subscriber only in tests for debugging

**Result:** Clean SDK experience with no leaked Rust debugging output.
