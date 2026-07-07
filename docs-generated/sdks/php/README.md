# PHP SDK

<!--
---
title: PHP SDK
description: PHP SDK for the Hyperswitch Prism payment orchestration platform
last_updated: 2026-03-21
sdk_language: php
---
-->
## What is Prism?

Prism is a stateless, unified connector library to connect with any payment processor. It is extracted out of the hardened integrations through continuous testing & iterative bug fixing over years of usage within [Juspay Hyperswitch](https://github.com/juspay/hyperswitch).


### Why are payment processor integrations such a big deal?

Every payment processor has diverse APIs, error codes, authentication methods, pdf documents to read, and behavioural differences between the actual environment and documented specs. 

A small mistake or oversight can create a huge financial impact for businesses accepting payments. Thousands of enterprises around the world have gone through this learning curve and iterated and fixed payment systems over many years. All such fixes/improvements/iterations are locked-in as tribal knowledge into Enterprise Payment Platforms and SaaS Payment Orchestration solutions. 

Hence, **Prism** - to open up payment diversity to the entire world as a simple, lightweight, zero lock-in, developer friendly payments library.

**Prism is extracted, built and maintained by the team behind [Juspay Hyperswitch](https://github.com/juspay/hyperswitch) - the open-source payments platform with 40K+ Github stars and used by leading enterprise merchants around the world.**

**Note:** In all honesty, payments are not more complicated than database drivers. It is simply just that the industry has not arrived at a standard (and it never will!!).


## What does Prism do well?
- **One request schema** for every payment. The same authorize call works against Stripe, Adyen and many more without additional lines of code.
- **Stateless. No database, no stored PII.** Credentials are not stored/ logged by the library. It lives only up to the lifetime of your HTTP client.
- **PCI scope reduction.** The card data flowing/ not flowing into the library is your choice. You can choose to leverage any payment processor vault or your own PCI certified vault. Nothing is logged or stored by the library.


## Integrations - Status

Prism supports **multiple connectors** with varying levels of payment method and flow coverage. Each connector is continuously tested against real sandbox/ production environments.

**Legend:** ✓ Supported | x Not Supported | ⚠ In Progress | ? Needs Validation

| Status | Description |
|--------|-------------|
| ✓ | Fully implemented and tested |
| x | Not applicable or unsupported by processor |
| ⚠ | Implementation in progress or partial |
| ? | Implementation needs validation against live environment |

**[View Complete Connector Coverage →](./docs-generated/all_connector.md)**

## What Prism does not do (yet)?
- **Built-in vault or tokenization service.** This is a design choice. You may bring your own vault, or use the payment processor's vault.
- **Retry or routing logic.** It lives in [Juspay Hyperswitch](https://github.com/juspay/hyperswitch). Prism is only the transformation layer.
- **Beyond payments.** The diversity exists beyond payments - in subscriptions, fraud, tax, payouts. And it is our aspiration, to evolve Prism into a stateless commerce library.
## Installation

```bash
composer require hyperswitch/prism
```

## Quick Start

```php
<?php
require_once 'vendor/autoload.php';

use HyperswitchPrism\PaymentClient;

$paymentClient = new PaymentClient([
    'connector' => 'stripe',
    'apiKey' => 'YOUR_API_KEY',
    'environment' => 'SANDBOX'
]);

// Authorize a payment
$response = $paymentClient->authorize([
    'merchantTransactionId' => 'txn_order_001',
    'amount' => [
        'minorAmount' => 1000,
        'currency' => 'USD'
    ],
    'paymentMethod' => [
        'card' => [
            'cardNumber' => ['value' => '4242424242424242'],
            'cardExpMonth' => ['value' => '12'],
            'cardExpYear' => ['value' => '2027'],
            'cardCvc' => ['value' => '123'],
            'cardHolderName' => ['value' => 'John Doe']
        ]
    ],
    'authType' => 'NO_THREE_DS'
]);

echo $response['status']; // AUTHORIZED
```

## Services

| Service | Description |
|---------|-------------|
| [Payment Service](./payment-service/README.md) | Process payments from authorization to settlement |
| [Recurring Payment Service](./recurring-payment-service/README.md) | Manage subscriptions and recurring billing |
| [Refund Service](./refund-service/README.md) | Retrieve and track refund statuses |
| [Dispute Service](./dispute-service/README.md) | Handle chargebacks and disputes |
| [Event Service](./event-service/README.md) | Process webhook notifications |
| [Payment Method Service](./payment-method-service/README.md) | Store and manage payment methods |
| [Customer Service](./customer-service/README.md) | Manage customer profiles |
| [Merchant Authentication Service](./merchant-authentication-service/README.md) | Generate access tokens |
| [Payment Method Authentication Service](./payment-method-authentication-service/README.md) | 3D Secure authentication |
| [Payout Service](./payout-service/README.md) | Send funds to recipients |

## Configuration

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `connector` | string | Yes | Payment connector name (stripe, adyen, etc.) |
| `apiKey` | string | Yes | Your API key |
| `environment` | string | Yes | SANDBOX or PRODUCTION |
| `timeout` | int | No | Request timeout in seconds (default: 30) |

## Error Handling

```php
try {
    $response = $paymentClient->authorize($request);
} catch (PaymentDeclinedException $e) {
    // Handle declined payment
    echo $e->getMessage();
} catch (ValidationException $e) {
    // Handle validation error
    echo $e->getErrors();
} catch (HyperswitchException $e) {
    // Handle other errors
    echo $e->getMessage();
}
```

## Support

For support and documentation, visit [https://docs.hyperswitch.io](https://docs.hyperswitch.io)
