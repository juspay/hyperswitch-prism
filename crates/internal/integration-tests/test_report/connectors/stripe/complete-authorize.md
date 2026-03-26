# Connector `stripe` / Suite `complete_authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `100.0%` (`1` / `1`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Credit Card \| 3DS`](./complete-authorize/threeds-complete-authorize-credit-card.md) | card | credit | `PASS` | `authorize(threeds_manual_capture_credit_card)` (PASS) |