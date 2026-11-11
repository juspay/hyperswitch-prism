# Dependency Context Mapping

This document explains how values flow from dependency suites into downstream requests.

## Why mapping exists

Dependency outputs and downstream request fields are not always shaped the same:

- `setup_recurring` returns `mandate_reference.*`
- `recurring_charge` expects `connector_recurring_payment_id.*`

Some fields are also nested differently (`state.access_token.*` vs top-level response fields),
and protobuf oneof wrappers add paths like `id_type.id`.

## Runtime order

For each downstream request:

1. `add_context` (implicit)
   - Collects dependency request/response JSON.
   - Fills unresolved request fields by path lookup and alias candidates.
2. `context_map` (explicit)
   - Applies per-dependency mappings declared in `suite_spec.json`.
   - Runs after implicit mapping, so explicit values take precedence.
3. Auto-generation/pruning
   - Remaining unresolved placeholders are generated or pruned depending on field type.

## `context_map` syntax

Inside `depends_on`:

```json
{
  "suite": "refund",
  "context_map": {
    "refund_id": "res.connector_refund_id"
  }
}
```

- Left side = target path in current request.
- Right side = source path from dependency payload.
- Prefixes:
  - `res.`: dependency response
  - `req.`: dependency request
  - no prefix: treated as `res.`

## When to add explicit mappings

Add `context_map` when:

- target/source names differ (`refund_id <- res.connector_refund_id`)
- target/source path shape differs and must be explicit
- multiple dependencies could satisfy a field and source must be pinned

Do not add it when:

- source and target are exact same-name and unambiguous (implicit mapping is enough)

## Current examples

- `src/global_suites/recurring_charge_suite/suite_spec.json`
  - explicit mandate mapping from `setup_recurring`
- `src/global_suites/refund_sync_suite/suite_spec.json`
  - explicit `refund_id <- res.connector_refund_id`
