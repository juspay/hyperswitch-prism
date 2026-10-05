# Auto-Populate Sanity Layer

## Why

A merchant's connector config isn't shaped like our typed config — it's their own structure. To use it, we need to read the merchant's raw config and drop the right values into the right fields of our request. That bridging step is the sanity layer.

## How it works

Done at build time, in a generator that runs before compilation:

```
.proto request messages
        │
        ▼
for each field we track (a short hardcoded list):
        │
        ▼
   does this message have the field
   (directly, or via a nested field)?
       │                │
      yes               no
       │                │
       ▼                ▼
  generate a real   generate nothing —
  populate function  message just gets
                      the trait's default
                      (no-op)
```

Every request message ends up implementing the same trait either way — with real logic if it can reach the field, or a free no-op if it can't.

## Dummy example

Field we track: `discount_code`, reachable only via `OrderRequest → PromoContext.discount_code`.

- `OrderRequest` has a nested `PromoContext` with `discount_code` → generator writes a real setter.
- `PaymentRequest` has no such field anywhere in it → generator gives it the default no-op.
- Caller calls `req.populate_discount_code(value)` on either — works correctly either way, no branching needed.

## What gets generated (real example)

```rust
pub trait PopulateOsBasedReturnUrl {
    fn populate_os_based_return_url(&mut self, value: OsBasedReturnUrl) {
        let _ = value; // default: no-op
    }
}

// has the field directly
impl PopulateOsBasedReturnUrl for AuthenticatorClientAuthenticationContext {
    fn populate_os_based_return_url(&mut self, value: OsBasedReturnUrl) {
        if let Some(existing) = self.os_based_return_url.as_mut() {
            if existing.os_type != 0 {
                existing.return_url_map = value.return_url_map;
            }
        }
    }
}

// no path to the field — uses the trait default
impl PopulateOsBasedReturnUrl for PaymentServiceVoidRequest {}
```

`os_type` is only ever read, never overwritten. `return_url_map` replaces, not merges.

## Using it at runtime

```
request ──▶ SanityLayer ──▶ resolve connector ──▶ has a sanity handler?
                                                    │
                                       ┌────────────┴────────────┐
                                      yes                        no
                                       │                          │
                                       ▼                          ▼
                                 apply() → populate_*()      skip, log, continue
```

Feature-gated (`connector-sanity-layer`) — off by default, zero runtime cost when disabled.

## Extending

- **New field:** add it to the `.proto` message + one variant in `AutoPopulateField`.
- **New connector:** implement `ConnectorSanity`, override only the `populate_*` hooks it has data for.
