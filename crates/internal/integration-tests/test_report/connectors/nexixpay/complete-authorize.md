# Connector `nexixpay` / Suite `complete_authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `0.0%` (`0` / `1`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`threeds_complete_authorize_credit_card`](./complete-authorize/threeds-complete-authorize-credit-card.md) | card | credit | `FAIL` | `pre_authenticate(threeds_card_pre_authenticate)` (FAIL) -> `post_authenticate(threeds_card_post_authenticate)` (FAIL) |

## Failed Scenarios

- [`threeds_complete_authorize_credit_card`](./complete-authorize/threeds-complete-authorize-credit-card.md) — assertion failed for field 'connector_transaction_id': expected field to exist
