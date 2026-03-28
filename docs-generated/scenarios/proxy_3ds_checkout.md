




# Proxy Payment with 3DS (VGS + Proxy 3DS)

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/manifest.json
Scenario: proxy_3ds_checkout
Regenerate: python3 scripts/generators/docs/generate.py --scenarios
-->

## Overview

Full 3DS flow using vault alias tokens routed through an outbound proxy. The proxy substitutes aliases before forwarding to Netcetera (3DS server). Authorize after successful authentication using the same vault aliases.

**Use this for:** 3D Secure authentication with vault tokens

**Flows used:** proxy_pre_authenticate, proxy_authenticate, proxy_post_authenticate, proxy_authorize





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
| 1 | ProxyPaymentService.PreAuthenticate | Initiate 3DS flow using vault alias tokens. The proxy substitutes aliases before forwarding to the 3DS server (e.g. Netcetera). | `ProxyPaymentMethodAuthenticationServicePreAuthenticateRequest` |
| 2 | ProxyPaymentService.Authenticate | Execute 3DS challenge or frictionless verification using vault alias tokens routed through a proxy. | `ProxyPaymentMethodAuthenticationServiceAuthenticateRequest` |
| 3 | ProxyPaymentService.PostAuthenticate | Validate 3DS authentication result with the issuing bank using vault alias tokens routed through a proxy. | `ProxyPaymentMethodAuthenticationServicePostAuthenticateRequest` |
| 4 | ProxyPaymentService.Authorize | Authorize using vault alias tokens (VGS / Basis Theory / Spreedly). The proxy substitutes aliases with real card values before forwarding to the connector. | `ProxyPaymentServiceAuthorizeRequest` |


## Common Issues

### "capture_method not supported"

Some connectors only support `AUTOMATIC` (single-step). Use the [Auto-Capture scenario](./checkout-autocapture.md) instead.

### Auth expires before capture

Capture timing varies by connector. Check the connector-specific documentation for auth window details.

## Related Scenarios



---

*This documentation was auto-generated from probe data. Last updated: 2026-03-24*