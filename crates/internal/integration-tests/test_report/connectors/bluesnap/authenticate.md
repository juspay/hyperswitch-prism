# Connector `bluesnap` / Suite `authenticate`

- Service: `PaymentMethodAuthenticationService/Authenticate`
- Pass Rate: `0.0%` (`0` / `1`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Card Authenticate \| 3DS`](./authenticate/threeds-card-authenticate.md) | card | credit | `FAIL` | `pre_authenticate(threeds_card_pre_authenticate)` (FAIL) |

## Failed Scenarios

- [`Card Authenticate | 3DS`](./authenticate/threeds-card-authenticate.md) — unsupported suite 'authenticate' for grpcurl generation