




# Tokenize Payment Method

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/manifest.json
Scenario: tokenize
Regenerate: python3 scripts/generators/docs/generate.py --scenarios
-->

## Overview

Store card details in the connector's vault and receive a reusable payment token. Use the returned token for one-click payments and recurring billing without re-collecting card data.

**Use this for:** Storing payment methods securely for future use

**Flows used:** tokenize





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
| **Billwerk** | ✅ Supported |
| **Braintree** | ✅ Supported |
| **Finix** | ✅ Supported |
| **HiPay** | ✅ Supported |
| **Paysafe** | ✅ Supported |
| **Stax** | ✅ Supported |
| **Stripe** | ✅ Supported |






## Connector Implementations

Complete, runnable examples for each connector:

### Python


<details>
<summary><b>Billwerk</b></summary>

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
<summary><b>Braintree</b></summary>

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
<summary><b>HiPay</b></summary>

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
<summary><b>Paysafe</b></summary>

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
<summary><b>Billwerk</b></summary>

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
<summary><b>Braintree</b></summary>

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
<summary><b>HiPay</b></summary>

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
<summary><b>Paysafe</b></summary>

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
<summary><b>Billwerk</b></summary>

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
<summary><b>Braintree</b></summary>

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
<summary><b>HiPay</b></summary>

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
<summary><b>Paysafe</b></summary>

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
<summary><b>Billwerk</b></summary>

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
<summary><b>Braintree</b></summary>

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
<summary><b>HiPay</b></summary>

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
<summary><b>Paysafe</b></summary>

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
| 1 | PaymentMethodService.Tokenize | Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing. | `PaymentMethodServiceTokenizeRequest` |


## Common Issues

### "capture_method not supported"

Some connectors only support `AUTOMATIC` (single-step). Use the [Auto-Capture scenario](./checkout-autocapture.md) instead.

### Auth expires before capture

Capture timing varies by connector. Check the connector-specific documentation for auth window details.

## Related Scenarios



---

*This documentation was auto-generated from probe data. Last updated: 2026-03-24*