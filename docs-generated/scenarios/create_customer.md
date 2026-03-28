




# Create Customer

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/manifest.json
Scenario: create_customer
Regenerate: python3 scripts/generators/docs/generate.py --scenarios
-->

## Overview

Register a customer record in the connector system. Returns a connector_customer_id that can be reused for recurring payments and tokenized card storage.

**Use this for:** Creating customer records for future payments

**Flows used:** create_customer





## Quick Start

Choose your SDK language to see a complete working example:

<table>
<tr><td><b>Python</b></td><td><b>JavaScript</b></td><td><b>Kotlin</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Python Quick Start</summary>

```python

```

</details>

</td>
<td valign="top">

<details><summary>JavaScript Quick Start</summary>

```javascript

```

</details>

</td>
<td valign="top">

<details><summary>Kotlin Quick Start</summary>

```kotlin

```

</details>

</td>
<td valign="top">

<details><summary>Rust Quick Start</summary>

```rust

```

</details>

</td>
</tr>
</table>

## Supported Connectors


This scenario is supported by the following connectors:

| Connector | Status |
|-----------|--------|
| **Authorize.net** | ✅ Supported |
| **Finix** | ✅ Supported |
| **Stax** | ✅ Supported |
| **Stripe** | ✅ Supported |






## Connector Implementations

Complete, runnable examples for each connector:

### Python


<details>
<summary><b>Authorize.net</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
# Example not available
```

</details>


<details>
<summary><b>Finix</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
# Example not available
```

</details>


<details>
<summary><b>Stax</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
# Example not available
```

</details>


<details>
<summary><b>Stripe</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
# Example not available
```

</details>



### Rust


<details>
<summary><b>Authorize.net</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
// Example not available
```

</details>


<details>
<summary><b>Finix</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
// Example not available
```

</details>


<details>
<summary><b>Stax</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
// Example not available
```

</details>


<details>
<summary><b>Stripe</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
// Example not available
```

</details>



### JavaScript


<details>
<summary><b>Authorize.net</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
// Example not available
```

</details>


<details>
<summary><b>Finix</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
// Example not available
```

</details>


<details>
<summary><b>Stax</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
// Example not available
```

</details>


<details>
<summary><b>Stripe</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
// Example not available
```

</details>



### Kotlin


<details>
<summary><b>Authorize.net</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
// Example not available
```

</details>


<details>
<summary><b>Finix</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
// Example not available
```

</details>


<details>
<summary><b>Stax</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
// Example not available
```

</details>


<details>
<summary><b>Stripe</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
// Example not available
```

</details>



## Flow Reference

This scenario uses these flows in sequence:

| Step | Flow (Service.RPC) | Purpose | gRPC Request |
|------|-------------------|---------|--------------|
| 1 | CustomerService.Create | Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information. | `CustomerServiceCreateRequest` |


## Common Issues

### "capture_method not supported"

Some connectors only support `AUTOMATIC` (single-step). Use the [Auto-Capture scenario](./checkout-autocapture.md) instead.

### Auth expires before capture

Capture timing varies by connector. Check the connector-specific documentation for auth window details.

## Related Scenarios



---

*This documentation was auto-generated from probe data. Last updated: 2026-03-24*