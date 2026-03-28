




# 3DS Authentication

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/manifest.json
Scenario: authentication
Regenerate: python3 scripts/generators/docs/generate.py --scenarios
-->

## Overview

Full 3D Secure authentication flow: PreAuthenticate collects device/browser data, Authenticate executes the challenge or frictionless verification, PostAuthenticate validates the result with the issuing bank.

**Use this for:** See scenario description

**Flows used:** pre_authenticate, authenticate, post_authenticate





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


⚠️ No connectors currently support this scenario.




## Connector Implementations

Complete, runnable examples for each connector:

### Python



### Rust



### JavaScript



### Kotlin



## Flow Reference

This scenario uses these flows in sequence:

| Step | Flow (Service.RPC) | Purpose | gRPC Request |
|------|-------------------|---------|--------------|
| 1 | PaymentMethodAuthenticationService.PreAuthenticate | Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification. | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| 2 | PaymentMethodAuthenticationService.Authenticate | Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention. | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| 3 | PaymentMethodAuthenticationService.PostAuthenticate | Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed. | `PaymentMethodAuthenticationServicePostAuthenticateRequest` |


## Common Issues

### "capture_method not supported"

Some connectors only support `AUTOMATIC` (single-step). Use the [Auto-Capture scenario](./checkout-autocapture.md) instead.

### Auth expires before capture

Capture timing varies by connector. Check the connector-specific documentation for auth window details.

## Related Scenarios



---

*This documentation was auto-generated from probe data. Last updated: 2026-03-24*