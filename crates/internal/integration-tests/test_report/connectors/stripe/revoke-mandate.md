# Connector `stripe` / Suite `revoke_mandate`

- Service: `Unknown`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`revoke_active_mandate`](./revoke-mandate/revoke-active-mandate.md) | - | - | `FAIL` | `setup_recurring(setup_recurring)` (PASS) |
| [`revoke_fail_invalid_mandate_id`](./revoke-mandate/revoke-fail-invalid-mandate-id.md) | - | - | `FAIL` | `setup_recurring(setup_recurring)` (PASS) |
| [`revoke_with_reason`](./revoke-mandate/revoke-with-reason.md) | - | - | `FAIL` | `setup_recurring(setup_recurring)` (PASS) |

## Failed Scenarios

- [`revoke_active_mandate`](./revoke-mandate/revoke-active-mandate.md) — Resolved method descriptor:
- [`revoke_fail_invalid_mandate_id`](./revoke-mandate/revoke-fail-invalid-mandate-id.md) — Resolved method descriptor:
- [`revoke_with_reason`](./revoke-mandate/revoke-with-reason.md) — Resolved method descriptor:
