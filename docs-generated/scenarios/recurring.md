




# Recurring / Mandate Payments

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/manifest.json
Scenario: recurring
Regenerate: python3 scripts/generators/docs/generate.py --scenarios
-->

## Overview

Store a payment mandate with SetupRecurring, then charge it repeatedly with RecurringPaymentService.Charge without requiring customer action.

**Use this for:** Setting up subscription payments or stored mandates

**Flows used:** setup_recurring, recurring_charge





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
| **ACI** | ✅ Supported |
| **Adyen** | ✅ Supported |
| **Authorize.net** | ✅ Supported |
| **Bambora APAC** | ✅ Supported |
| **Checkout.com** | ✅ Supported |
| **CyberSource** | ✅ Supported |
| **Novalnet** | ✅ Supported |
| **PayPal** | ✅ Supported |
| **Stripe** | ✅ Supported |





## Status Handling


### Setup_recurring Flow

| Status | Recommended Action |
|--------|-------------------|
| `PENDING` | Mandate stored — save connector_transaction_id for future RecurringPaymentService.Charge calls |
| `FAILED` | Setup failed — customer must re-enter payment details |





## Connector Implementations

Complete, runnable examples for each connector:

### Python


<details>
<summary><b>ACI</b></summary>

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
<summary><b>Adyen</b></summary>

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
<summary><b>Bambora APAC</b></summary>

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
<summary><b>Checkout.com</b></summary>

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
<summary><b>CyberSource</b></summary>

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
<summary><b>Novalnet</b></summary>

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
<summary><b>PayPal</b></summary>

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
<summary><b>ACI</b></summary>

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
<summary><b>Adyen</b></summary>

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
<summary><b>Bambora APAC</b></summary>

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
<summary><b>Checkout.com</b></summary>

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
<summary><b>CyberSource</b></summary>

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
<summary><b>Novalnet</b></summary>

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
<summary><b>PayPal</b></summary>

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
<summary><b>ACI</b></summary>

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
<summary><b>Adyen</b></summary>

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
<summary><b>Bambora APAC</b></summary>

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
<summary><b>Checkout.com</b></summary>

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
<summary><b>CyberSource</b></summary>

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
<summary><b>Novalnet</b></summary>

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
<summary><b>PayPal</b></summary>

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
<summary><b>ACI</b></summary>

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
<summary><b>Adyen</b></summary>

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
<summary><b>Bambora APAC</b></summary>

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
<summary><b>Checkout.com</b></summary>

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
<summary><b>CyberSource</b></summary>

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
<summary><b>Novalnet</b></summary>

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
<summary><b>PayPal</b></summary>

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
| 1 | PaymentService.SetupRecurring | Setup a recurring payment instruction for future payments/ debits. This could be for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases. | `PaymentServiceSetupRecurringRequest` |
| 2 | RecurringPaymentService.Charge | Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details. | `RecurringPaymentServiceChargeRequest` |


## Common Issues

### "capture_method not supported"

Some connectors only support `AUTOMATIC` (single-step). Use the [Auto-Capture scenario](./checkout-autocapture.md) instead.

### Auth expires before capture

Capture timing varies by connector. Check the connector-specific documentation for auth window details.

## Related Scenarios



---

*This documentation was auto-generated from probe data. Last updated: 2026-03-24*