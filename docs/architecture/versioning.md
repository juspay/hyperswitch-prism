# Versioning

Prism follows [Semantic Versioning 2.0.0](https://semver.org/). A minor version upgrade or a patch will never break your existing integration.

It is recommended to **pin to `MAJOR.MINOR.*`. Get fixes automatically. Control features manually. Sleep soundly.**

```
MAJOR.MINOR.PATCH
  1    .2    .3
```

## Version Number Meanings

| Position | When It Changes | What It Means for You |
|----------|-----------------|----------------------|
| **PATCH** (1.2.3 → 1.2.4) | Bug fixes, security patches | Update automatically. Zero code changes required. |
| **MINOR** (1.2.x → 1.3.x) | New features, new connectors | Add capabilities without touching existing code. |
| **MAJOR** (1.x.x → 2.x.x) | Breaking API changes | You must update your code. Migration guide provided. |

## Pinning for Automatic Bug Fixes

It is strongly recommended pull security patches and critical fixes without manual updates. Pin your dependency to accept patch increments automatically.

<!-- tabs:start -->

#### **Node.js**

```json
{
  "dependencies": {
    "@juspay-tech/hyperswitch-prism": "1.2.*"
  }
}
```

This accepts: `1.2.0`, `1.2.1`, `1.2.4`, `1.2.15`
This rejects: `1.3.0`, `2.0.0`

#### **Python**

```text
hyperswitch-prism==1.2.*
```

Or in `pyproject.toml`:

```toml
[tool.poetry.dependencies]
hyperswitch-prism = "1.2.*"
```

#### **Java**

```xml
<dependency>
    <groupId>com.juspay</groupId>
    <artifactId>hyperswitch-prism</artifactId>
    <version>[1.2.0,1.3.0)</version>
</dependency>
```

The `[1.2.0,1.3.0)` syntax means: 1.2.0 inclusive, 1.3.0 exclusive.

#### **PHP**

```json
{
  "require": {
    "juspay/hyperswitch-prism": "1.2.*"
  }
}
```

This accepts any `1.2.x` version but not `1.3.0` or `2.0.0`.

<!-- tabs:end -->

## What You Get Automatically

When you pin to `1.2.*`, your build system pulls these automatically:

**Patch releases (automatic):**
- Security fixes for connector authentication
- Bug fixes for specific PSP error parsing
- Performance improvements
- Documentation corrections

**Minor releases (manual opt-in):**
- New connector support (e.g., "Added Peach Payments")
- New payment methods (e.g., "Added UPI")
- New SDK features (e.g., "Added async streaming")
- Deprecation warnings for old APIs

**Major releases (manual migration):**
- Breaking changes to core types
- Removal of deprecated methods
- Fundamental architecture changes

## The Risk of Pinning Too Tightly

```json
{
  "dependencies": {
    "hyperswitch-prism": "1.2.3"
  }
}
```

This pins exactly to `1.2.3`. You miss:
- `1.2.4` — Fix for Stripe webhook signature verification
- `1.2.5` — Security patch for Adyen credential handling
- `1.2.6` — Critical fix for refund idempotency keys

Your code works today. It breaks tomorrow when Stripe rotates certificates and you lack the fix.

## The Risk of Pinning Too Loosely

```json
{
  "dependencies": {
    "hyperswitch-prism": "*"
  }
}
```

This accepts any version, including `2.0.0` with breaking changes. Your CI passes today. Production fails tomorrow when a new major version introduces API changes.

## Recommended Strategy

It is strongly recommended to pin to minor version for active development:

```
1.2.*
```

This gives you the ability to:
- Automatic security patches
- Automatic bug fixes
- Control over when new features arrive

You may update minor versions intentionally when you need new connectors or features. 
Read the changelog. Run your integration tests. Bump the pin.

## Version Compatibility Matrix

Prism maintains compatibility across SDK languages for the same minor version:

| Prism Version | Node.js SDK | Python SDK | Java SDK | PHP SDK |
|---------------------------|-------------|------------|----------|---------|
| 1.2.x | 1.2.x | 1.2.x | 1.2.x | 1.2.x |
| 1.3.x | 1.3.x | 1.3.x | 1.3.x | 1.3.x |

All SDKs for version `1.2.x` speak the same protocol, support the same connectors, and handle the same error codes. Mixing SDK versions (Node.js at `1.2.5`, Python at `1.3.0`) works but may produce different behaviors for newer features.

## Checking Your Current Version

```bash
# Node.js
npm list @juspay-tech/hyperswitch-prism

# Python
pip show hyperswitch-prism

# Java
mvn dependency:tree | grep hyperswitch-prism

# PHP
composer show juspay/hyperswitch-prism
```

## Deprecation Policy

Prism maintains deprecated APIs for one full major version. When `2.0.0` releases:
- APIs deprecated in `1.x` are removed
- Migration guides are published
- Automated codemods are provided where possible

You have the entire `1.x` lifecycle to update your code before breaking changes arrive.
