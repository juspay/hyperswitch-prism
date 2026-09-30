---
name: New connector integration (Hyperswitch)
about: End-to-end checklist for adding a payment connector to Hyperswitch
title: "feat(connector): add <connector-name>"
labels: ["integration"]
---

## Connector

- **Name**: <!-- e.g. razorpay -->
- **API documentation**: <!-- link -->
- **PR(s)**:

## Checklist

- [ ] Regression
- [ ] Integrity
- [ ] Required fields
- [ ] TOML files
- [ ] Feature matrix
- [ ] Whitelisting the URL
- [ ] GSM
- [ ] Wasm change
- [ ] Dashboard changes

## What each item means

**Required fields**

Dynamic required fields which the SDK is supposed to render is to be added both in code
and superposition.

**TOML files**

Supported currencies and billing countries should be added to the TOML if they are stated
in the doc. If there is no information, we can try to find it or not add the filters for
the connector.

**Feature matrix**

Ensure the enhancement or integration has the feature matrix updated.

**Whitelisting the URL**

Enable the URL in the proxy layer.

**GSM**

Adding required GSM codes in the DB.

## Dashboard

Who is raising the dashboard change?

- [ ] Raised by us
- [ ] Needs dashboard team — request raised: <!-- link -->
