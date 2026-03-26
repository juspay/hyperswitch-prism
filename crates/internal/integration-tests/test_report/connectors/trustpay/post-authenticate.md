# Connector `trustpay` / Suite `post_authenticate`

- Service: `PaymentMethodAuthenticationService/PostAuthenticate`
- Pass Rate: `0.0%` (`0` / `1`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Card Post Authenticate \| 3DS`](./post-authenticate/threeds-card-post-authenticate.md) | card | credit | `FAIL` | `authenticate(threeds_card_authenticate)` (FAIL) |

## Failed Scenarios

- [`Card Post Authenticate | 3DS`](./post-authenticate/threeds-card-post-authenticate.md) — Resolved method descriptor: