# medusa-unified-payment

Hyperswitch Prism payment integration for Medusa v2. This monorepo contains two packages:

| Package | Description |
|---------|-------------|
| [`@juspay-tech/medusa-unified-payment`](./medusa-unified-payment) | Medusa v2 backend payment provider plugin |
| [`@juspay-tech/medusa-unified-payment-react`](./medusa-unified-payment-react) | React UI components for storefront checkout |

## Supported Connectors

> **Powered by [Hyperswitch Prism](https://github.com/juspay/hyperswitch-prism)** — a unified payment orchestration layer that routes transactions across processors through a single API.
>
> | Connector | `@juspay-tech/medusa-unified-payment` | `@juspay-tech/medusa-unified-payment-react` |
> |-----------|:---------------------------------:|:--------------------------------------:|
> | [![Stripe](https://img.shields.io/badge/Stripe-626CD9?style=for-the-badge&logo=stripe&logoColor=white)](https://stripe.com) | ✅ | ✅ |
> | [![Adyen](https://img.shields.io/badge/Adyen-0ABF53?style=for-the-badge&logo=adyen&logoColor=white)](https://adyen.com) | ✅ | ✅ |
> | [![PayPal](https://img.shields.io/badge/PayPal-003087?style=for-the-badge&logo=paypal&logoColor=white)](https://paypal.com) | ✅ | ✅ |
> | [![GlobalPay](https://img.shields.io/badge/GlobalPay-E4002B?style=for-the-badge&logoColor=white)](https://developer.globalpay.com) | ✅ | ✅ |
> | [![Braintree](https://img.shields.io/badge/Braintree-1B3FA0?style=for-the-badge&logoColor=white)](https://developer.paypal.com/braintree/docs) | ✅ | — |
> | [![Cybersource](https://img.shields.io/badge/Cybersource-FF6600?style=for-the-badge&logoColor=white)](https://developer.cybersource.com) | ✅ | — |
> | [![Mollie](https://img.shields.io/badge/Mollie-000000?style=for-the-badge&logo=mollie&logoColor=white)](https://mollie.com) | ✅ | — |


## Quick Start

### Backend

```bash
npm install @juspay-tech/medusa-unified-payment
```

See [`medusa-unified-payment/README.md`](./medusa-unified-payment/README.md) for full setup.

### Storefront

```bash
npm install @juspay-tech/medusa-unified-payment-react
```

See [`medusa-unified-payment-react/README.md`](./medusa-unified-payment-react/README.md) for full setup.

## Development

```bash
# Install dependencies and build both packages
npm install
npm run build                          # builds @juspay-tech/medusa-unified-payment
cd medusa-unified-payment-react && npm install && npm run build
```

## License

MIT
