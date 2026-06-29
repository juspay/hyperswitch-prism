# medusa-custom-payments

Hyperswitch Prism payment integration for Medusa v2. This monorepo contains two packages:

| Package | npm | Description |
|---------|-----|-------------|
| [`@juspay-tech/medusa-custom-payments`](./medusa-custom-payments) | [![npm](https://img.shields.io/npm/v/@juspay-tech/medusa-custom-payments?logo=npm)](https://www.npmjs.com/package/@juspay-tech/medusa-custom-payments) | Medusa v2 backend payment provider plugin |
| [`@juspay-tech/medusa-custom-payments-react`](./medusa-custom-payments-react) | [![npm](https://img.shields.io/npm/v/@juspay-tech/medusa-custom-payments-react?logo=npm)](https://www.npmjs.com/package/@juspay-tech/medusa-custom-payments-react) | React UI components for storefront checkout |

## Supported Connectors

> **Powered by [Hyperswitch Prism](https://github.com/juspay/hyperswitch-prism)** — a unified payment orchestration layer that routes transactions across processors through a single API.
>
> | Connector | `@juspay-tech/medusa-custom-payments` | `@juspay-tech/medusa-custom-payments-react` |
> |-----------|:---------------------------------:|:--------------------------------------:|
> | [![Stripe](https://img.shields.io/badge/Stripe-626CD9?style=for-the-badge&logo=stripe&logoColor=white)](https://stripe.com) | ✅ | ✅ |
> | [![Adyen](https://img.shields.io/badge/Adyen-0ABF53?style=for-the-badge&logo=adyen&logoColor=white)](https://adyen.com) | ✅ | ✅ |
> | [![PayPal](https://img.shields.io/badge/PayPal-003087?style=for-the-badge&logo=paypal&logoColor=white)](https://paypal.com) | ✅ | ✅ |
> | [![GlobalPay](https://img.shields.io/badge/GlobalPay-E4002B?style=for-the-badge&logoColor=white)](https://developer.globalpay.com) | ✅ | ✅ |
> | [![Braintree](https://img.shields.io/badge/Braintree-1B3FA0?style=for-the-badge&logoColor=white)](https://developer.paypal.com/braintree/docs) | ✅ | ✅ |
> | [![Cybersource](https://img.shields.io/badge/Cybersource-FF6600?style=for-the-badge&logoColor=white)](https://developer.cybersource.com) | ✅ | ✅ |
> | [![Mollie](https://img.shields.io/badge/Mollie-000000?style=for-the-badge&logo=mollie&logoColor=white)](https://mollie.com) | ✅ | ✅ |
> | [![Authorize.net](https://img.shields.io/badge/Authorize.Net-EE3124?style=for-the-badge&logoColor=white)](https://developer.authorize.net) | ✅ | ✅ |
> | [![PayU](https://img.shields.io/badge/PayU-A6CE39?style=for-the-badge&logoColor=white)](https://payu.com) | [🚧][payu-issue] | [🚧][payu-issue] |
> | [![Razorpay](https://img.shields.io/badge/Razorpay-3395FF?style=for-the-badge&logo=razorpay&logoColor=white)](https://razorpay.com) | [🚧][razorpay-issue] | [🚧][razorpay-issue] |
> | [![Square](https://img.shields.io/badge/Square-3E4348?style=for-the-badge&logo=square&logoColor=white)](https://developer.squareup.com) | [🚧][square-issue] | [🚧][square-issue] |
>
> ✅ Supported &nbsp;·&nbsp; 🚧 In Progress &nbsp;·&nbsp; — Not yet available
>
> [auth-issue]: https://github.com/juspay/hyperswitch-prism/issues/1527
> [payu-issue]: https://github.com/juspay/hyperswitch-prism/issues/1528
> [razorpay-issue]: https://github.com/juspay/hyperswitch-prism/issues/1529
> [square-issue]: https://github.com/juspay/hyperswitch-prism/issues/1530


## Quick Start

### Backend

```bash
npm install @juspay-tech/medusa-custom-payments
```

📦 [View on npm](https://www.npmjs.com/package/@juspay-tech/medusa-custom-payments)

See [`medusa-custom-payments/README.md`](./medusa-custom-payments/README.md) for full setup.

### Storefront

```bash
npm install @juspay-tech/medusa-custom-payments-react
```

📦 [View on npm](https://www.npmjs.com/package/@juspay-tech/medusa-custom-payments-react)

See [`medusa-custom-payments-react/README.md`](./medusa-custom-payments-react/README.md) for full setup.

## Development

```bash
# Install dependencies and build both packages
npm install
npm run build                          # builds @juspay-tech/medusa-custom-payments
cd medusa-custom-payments-react && npm install && npm run build
```

## License

Apache-2.0
