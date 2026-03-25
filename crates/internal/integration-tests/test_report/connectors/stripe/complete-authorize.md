# Connector `stripe` / Suite `complete_authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `100.0%` (`1` / `1`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`threeds_complete_authorize_credit_card`](./complete-authorize/threeds-complete-authorize-credit-card.md) | card | credit | `PASS` | `authorize(threeds_manual_capture_credit_card)` (PASS) |
