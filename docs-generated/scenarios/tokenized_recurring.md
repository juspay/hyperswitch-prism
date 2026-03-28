




# Tokenized Recurring Payments

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/manifest.json
Scenario: tokenized_recurring
Regenerate: python3 scripts/generators/docs/generate.py --scenarios
-->

## Overview

Store a payment mandate using a connector token with SetupRecurring, then charge it repeatedly with RecurringPaymentService without requiring customer action or re-collecting card data.

**Use this for:** Setting up subscriptions with stored tokens

**Flows used:** tokenized_setup_recurring, recurring_charge





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
| **Stripe** | ✅ Supported |






## Connector Implementations

Complete, runnable examples for each connector:

### Python


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
| 1 | TokenizedPaymentService.SetupRecurring | Store a payment mandate using a connector-issued token for future recurring charges without re-collecting card data. | `TokenizedPaymentServiceSetupRecurringRequest` |
| 2 | RecurringPaymentService.Charge | Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details. | `RecurringPaymentServiceChargeRequest` |


## Common Issues

### "capture_method not supported"

Some connectors only support `AUTOMATIC` (single-step). Use the [Auto-Capture scenario](./checkout-autocapture.md) instead.

### Auth expires before capture

Capture timing varies by connector. Check the connector-specific documentation for auth window details.

## Related Scenarios



---

*This documentation was auto-generated from probe data. Last updated: 2026-03-24*