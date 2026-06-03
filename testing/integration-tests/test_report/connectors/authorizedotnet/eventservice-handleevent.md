# Connector `authorizedotnet` / Suite `EventService/HandleEvent`

- Service: `EventService/HandleEvent`
- Pass Rate: `0.0%` (`0` / `4`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Handle Event \| Invalid Signature Test`](./eventservice-handleevent/invalid-signature.md) | - | - | `FAIL` | None |
| [`Handle Event \| Payment Failed`](./eventservice-handleevent/payment-failed.md) | - | - | `FAIL` | None |
| [`Handle Event \| Payment Succeeded`](./eventservice-handleevent/payment-succeeded.md) | - | - | `FAIL` | None |
| [`Handle Event \| Refund Succeeded`](./eventservice-handleevent/refund-succeeded.md) | - | - | `FAIL` | None |

## Failed Scenarios

- [`Handle Event | Invalid Signature Test`](./eventservice-handleevent/invalid-signature.md) — Resolved method descriptor:
- [`Handle Event | Payment Failed`](./eventservice-handleevent/payment-failed.md) — Resolved method descriptor:
- [`Handle Event | Payment Succeeded`](./eventservice-handleevent/payment-succeeded.md) — assertion failed for field 'source_verified': expected true, got missing
- [`Handle Event | Refund Succeeded`](./eventservice-handleevent/refund-succeeded.md) — assertion failed for field 'source_verified': expected true, got missing
