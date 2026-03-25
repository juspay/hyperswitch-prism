# Connector `elavon` / Suite `post_authenticate`

- Service: `PaymentMethodAuthenticationService/PostAuthenticate`
- Pass Rate: `0.0%` (`0` / `1`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`threeds_card_post_authenticate`](./post-authenticate/threeds-card-post-authenticate.md) | card | credit | `FAIL` | `authenticate(threeds_card_authenticate)` (FAIL) |

## Failed Scenarios

- [`threeds_card_post_authenticate`](./post-authenticate/threeds-card-post-authenticate.md) — unsupported suite 'post_authenticate' for grpcurl generation
